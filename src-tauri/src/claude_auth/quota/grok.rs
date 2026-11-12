//! SuperGrok billing's gRPC-web response. Only the reference's identified field paths are accepted.

use super::ClaudeQuotaWindow;
use std::{io::Read, time::Duration};
pub(super) const ENDPOINT: &str =
    "https://grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig";

pub(super) fn fetch(url: &str, token: &str, now: i64) -> Result<ClaudeQuotaWindow, String> {
    let client = crate::outbound_proxy::apply_blocking(reqwest::blocking::Client::builder()).map_err(|error| error.to_string())?
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "无法初始化 Claude SuperGrok 额度连接")?;
    let mut response = client
        .post(url)
        .bearer_auth(token)
        .header("origin", "https://grok.com")
        .header("referer", "https://grok.com/?_s=usage")
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .header("x-user-agent", "connect-es/2.1.1")
        .header("user-agent", "Agent-Switchboard-Claude")
        .body(vec![0; 5])
        .send()
        .map_err(|_| "Claude SuperGrok 额度连接失败，请稍后重试")?;
    if !response.status().is_success() {
        return Err(super::super::http::status_error(response.status().as_u16()));
    }
    if let Some(status) = response.headers().get("grpc-status") {
        check_status(status.to_str().map_err(|_| "SuperGrok gRPC 状态无效")?)?;
    }
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| "SuperGrok 额度响应读取失败")?;
    if bytes.len() > 1_048_576 {
        return Err("SuperGrok 额度响应超过大小限制".into());
    }
    parse(&bytes, now)
}

fn check_status(status: &str) -> Result<(), String> {
    let status: u32 = status
        .trim()
        .parse()
        .map_err(|_| "SuperGrok gRPC 状态无效")?;
    match status {
        0 => Ok(()),
        7 | 16 => Err("Claude SuperGrok 授权或订阅权限不可用，请重新登录或检查订阅".into()),
        4 | 8 | 13 | 14 => Err(format!(
            "Claude SuperGrok 暂时无法查询额度（gRPC {status}），请重试"
        )),
        code => Err(format!("Claude SuperGrok 额度查询被拒绝（gRPC {code}）")),
    }
}

fn data_frames(bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    if bytes
        .first()
        .is_some_and(|value| *value != 0 && *value != 0x80)
    {
        return Ok(vec![bytes]);
    }
    let mut index = 0;
    let mut frames = Vec::new();
    while index < bytes.len() {
        let header = bytes
            .get(index..index + 5)
            .ok_or("SuperGrok gRPC 帧头截断")?;
        let length = u32::from_be_bytes(
            header[1..]
                .try_into()
                .map_err(|_| "SuperGrok gRPC 长度无效")?,
        ) as usize;
        index += 5;
        let end = index.checked_add(length).ok_or("SuperGrok gRPC 长度溢出")?;
        let payload = bytes.get(index..end).ok_or("SuperGrok gRPC 数据截断")?;
        match header[0] {
            0 => frames.push(payload),
            0x80 => {
                let text =
                    std::str::from_utf8(payload).map_err(|_| "SuperGrok gRPC 尾帧不是 UTF-8")?;
                for line in text.lines() {
                    if let Some((key, value)) = line.split_once(':') {
                        if key.eq_ignore_ascii_case("grpc-status") {
                            check_status(value)?;
                        }
                    }
                }
            }
            _ => return Err("SuperGrok gRPC 帧类型不受支持".into()),
        }
        if frames.len() > 64 {
            return Err("SuperGrok gRPC 数据帧过多".into());
        }
        index = end;
    }
    Ok(frames)
}

#[derive(Default)]
struct Values {
    percent: Option<f64>,
    reset_seconds: Option<u64>,
    period: Option<u64>,
}

fn varint(bytes: &[u8], offset: &mut usize) -> Result<u64, String> {
    let mut value = 0_u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(*offset).ok_or("SuperGrok protobuf 整数截断")?;
        *offset += 1;
        if shift == 63 && byte > 1 {
            return Err("SuperGrok protobuf 整数溢出".into());
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("SuperGrok protobuf 整数溢出".into())
}

fn scan(bytes: &[u8], path: &[u64], values: &mut Values) -> Result<(), String> {
    let mut offset = 0;
    while offset < bytes.len() {
        let key = varint(bytes, &mut offset)?;
        let field = key >> 3;
        if field == 0 {
            return Err("SuperGrok protobuf 字段编号无效".into());
        }
        let mut next = path.to_vec();
        next.push(field);
        match key & 7 {
            0 => {
                let value = varint(bytes, &mut offset)?;
                match next.as_slice() {
                    [1, 5, 1] => values.reset_seconds = Some(value),
                    [1, 6, 1] => values.period = Some(value),
                    _ => {}
                }
            }
            1 => {
                bytes
                    .get(offset..offset + 8)
                    .ok_or("SuperGrok protobuf 数据截断")?;
                offset += 8;
            }
            2 => {
                let length = usize::try_from(varint(bytes, &mut offset)?)
                    .map_err(|_| "SuperGrok protobuf 长度无效")?;
                let end = offset
                    .checked_add(length)
                    .ok_or("SuperGrok protobuf 长度溢出")?;
                let nested = bytes
                    .get(offset..end)
                    .ok_or("SuperGrok protobuf 数据截断")?;
                if matches!(next.as_slice(), [1] | [1, 5] | [1, 6]) {
                    scan(nested, &next, values)?;
                }
                offset = end;
            }
            5 => {
                let raw: [u8; 4] = bytes
                    .get(offset..offset + 4)
                    .ok_or("SuperGrok protobuf 数据截断")?
                    .try_into()
                    .map_err(|_| "SuperGrok 百分比无效")?;
                if next == [1, 1] {
                    values.percent = Some(f32::from_le_bytes(raw).into());
                }
                offset += 4;
            }
            _ => return Err("SuperGrok protobuf 数据类型不受支持".into()),
        }
    }
    Ok(())
}

fn parse(bytes: &[u8], now: i64) -> Result<ClaudeQuotaWindow, String> {
    let mut values = Values::default();
    for frame in data_frames(bytes)? {
        scan(frame, &[], &mut values)?;
    }
    let reset = values
        .reset_seconds
        .and_then(|value| i64::try_from(value).ok())
        .and_then(|seconds| seconds.checked_mul(1000));
    let known_zero_period = reset.is_some_and(|reset| reset > now)
        && values
            .period
            .is_some_and(|period| (1..=4).contains(&period));
    let percent = values
        .percent
        .or(known_zero_period.then_some(0.0))
        .filter(|number| number.is_finite() && (0.0..=100.0).contains(number))
        .ok_or("SuperGrok 额度响应没有已识别的百分比字段；未将未知额度当作零使用")?;
    Ok(ClaudeQuotaWindow {
        id: "grok-credits".into(),
        label: "SuperGrok 订阅额度".into(),
        used_percent: Some(percent),
        remaining: None,
        limit: None,
        unlimited: false,
        unit: "percent",
        resets_at_ms: reset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload(percent: f32) -> Vec<u8> {
        [vec![10, 5, 13], percent.to_le_bytes().to_vec()].concat()
    }
    fn frame(flags: u8, value: &[u8]) -> Vec<u8> {
        [
            vec![flags],
            (value.len() as u32).to_be_bytes().to_vec(),
            value.to_vec(),
        ]
        .concat()
    }
    #[test]
    fn recognizes_known_paths_but_never_guesses_credits_from_unrelated_float_fields() {
        assert_eq!(
            parse(&frame(0, &payload(37.5)), 0).unwrap().used_percent,
            Some(37.5)
        );
        assert_eq!(parse(&payload(25.0), 0).unwrap().used_percent, Some(25.0));
        let mut unrelated = payload(50.0);
        unrelated[0] = 18;
        assert!(parse(&unrelated, 0).is_err());
        assert!(parse(&payload(f32::NAN), 0).is_err());
    }
    #[test]
    fn rejects_truncated_frames_and_error_trailers_even_if_a_numeric_data_frame_preceded_them() {
        assert!(parse(&[0, 0, 0, 0, 20, 1], 0).is_err());
        let data = [
            frame(0, &payload(50.0)),
            frame(0x80, b"grpc-status: 16\r\ngrpc-message: private-token"),
        ]
        .concat();
        let error = parse(&data, 0).unwrap_err();
        assert!(!error.contains("private-token"));
    }
}
