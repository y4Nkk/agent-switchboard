const MAX_CUSTOM_PROMPT_CHARS: usize = 4000;
pub(crate) const CUSTOM_QUESTION_ID: &str = "custom";
/// Stored with each batch; the version also owns the answer format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ProbeGrading {
    #[serde(rename = "1")]
    Integer,
    #[serde(rename = "svg-visual-v1")]
    SvgVisual,
    #[serde(rename = "html-visual-v1")]
    HtmlVisual,
}

impl ProbeGrading {
    pub(crate) fn custom_timeout_seconds(self) -> u64 {
        match self {
            Self::Integer => 5 * 60,
            Self::SvgVisual => 10 * 60,
            Self::HtmlVisual => 15 * 60,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "1",
            Self::SvgVisual => "svg-visual-v1",
            Self::HtmlVisual => "html-visual-v1",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "1" => Some(Self::Integer),
            "svg-visual-v1" => Some(Self::SvgVisual),
            "html-visual-v1" => Some(Self::HtmlVisual),
            _ => None,
        }
    }
}

/// The backend-owned question catalog, including its output contract. The
/// first entry is the radar's default selection.
pub(crate) struct ProbeQuestion {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) prompt: &'static str,
    pub(super) expected_answer: &'static str,
    pub(crate) grading: ProbeGrading,
    /// A hand-made reference rendering shown in the UI; never used for
    /// grading — visual answers stay human-observed.
    pub(crate) reference_visual: Option<&'static str>,
    /// Upper bound of runs per batch for this question, owned by the
    /// catalog: one human-observed animation is enough for the pelican.
    pub(crate) max_runs: u32,
    pub(crate) timeout_seconds: u64,
}

pub(crate) const QUESTIONS: [ProbeQuestion; 4] = [
    ProbeQuestion {
        id: "pelican-riding-bicycle",
        label: "鹈鹕骑自行车",
        prompt: "生成html，内容是svg绘制鹈鹕骑自行车2D动画，不用进行测试。\n直接在最终回答中输出完整 HTML 源码，不要只返回文件路径或完成说明。",
        expected_answer: "",
        grading: ProbeGrading::HtmlVisual,
        reference_visual: Some(include_str!("pelican-reference.html")),
        max_runs: 1,
        timeout_seconds: 15 * 60,
    },
    ProbeQuestion {
        id: "divisible-or-contains-3",
        label: "计数题",
        prompt: "从 1 到 100（含两端）的整数中，有多少个数能被 3 整除，或者十进制写法中至少含有一个数字 3？只输出最终数字。",
        expected_answer: "45",
        grading: ProbeGrading::Integer,
        reference_visual: None,
        max_runs: super::MAX_RUN_COUNT,
        timeout_seconds: 3 * 60,
    },
    ProbeQuestion {
        id: "mountain-round-trip",
        label: "行程题",
        prompt: "某人沿同一条山路上山后原路下山：上山速度 5 km/h，下山速度 7 km/h，往返一共用了 4 小时 48 分钟。山路单程长多少公里？只输出最终数字。",
        expected_answer: "14",
        grading: ProbeGrading::Integer,
        reference_visual: None,
        max_runs: super::MAX_RUN_COUNT,
        timeout_seconds: 3 * 60,
    },
    ProbeQuestion {
        id: "round-table-neighbours",
        label: "圆桌题",
        prompt: "5 个人围一张圆桌就坐（旋转后相同的坐法视为同一种），其中甲、乙两人必须相邻。共有多少种不同的坐法？只输出最终数字。",
        expected_answer: "12",
        grading: ProbeGrading::Integer,
        reference_visual: None,
        max_runs: super::MAX_RUN_COUNT,
        timeout_seconds: 3 * 60,
    },
];

/// The question one probe runs, after request validation. The label, full
/// text, and expected answer are snapshotted into the batch record so later
/// catalog edits never rewrite history.
pub(crate) struct ResolvedQuestion {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(super) prompt: String,
    pub(super) expected_answer: String,
    pub(crate) grading: ProbeGrading,
    pub(crate) max_runs: u32,
    pub(crate) timeout_seconds: u64,
}

impl ResolvedQuestion {
    pub(super) fn execution_prompt(&self) -> String {
        match self.grading {
            ProbeGrading::Integer => format!(
                "{}\n只输出最终的非负整数答案，不要解释或使用 Markdown。", self.prompt
            ),
            ProbeGrading::SvgVisual | ProbeGrading::HtmlVisual => self.prompt.clone(),
        }
    }
}

pub(crate) fn resolve_question(
    id: &str,
    custom_question: Option<String>,
    custom_answer: Option<String>,
    custom_grading: Option<ProbeGrading>,
) -> Result<ResolvedQuestion, String> {
    if id == CUSTOM_QUESTION_ID {
        let prompt = custom_question.unwrap_or_default().trim().to_string();
        if prompt.is_empty() {
            return Err("自定义题目不能为空".to_string());
        }
        if prompt.chars().count() > MAX_CUSTOM_PROMPT_CHARS {
            return Err(format!("自定义题目超过 {MAX_CUSTOM_PROMPT_CHARS} 字上限"));
        }
        let grading = custom_grading.ok_or("自定义题目需要选择回答方式")?;
        let answer = resolve_custom_answer(grading, custom_answer)?;
        return Ok(ResolvedQuestion {
            id: CUSTOM_QUESTION_ID.to_string(),
            label: match grading {
                ProbeGrading::Integer => "自定义题",
                ProbeGrading::SvgVisual => "自定义 SVG 题",
                ProbeGrading::HtmlVisual => "自定义 HTML 动画题",
            }.to_string(),
            prompt,
            expected_answer: answer,
            grading,
            max_runs: super::MAX_RUN_COUNT,
            timeout_seconds: grading.custom_timeout_seconds(),
        });
    }
    let question = QUESTIONS
        .iter()
        .find(|question| question.id == id)
        .ok_or_else(|| "未知的探针题目".to_string())?;
    Ok(ResolvedQuestion {
        id: question.id.to_string(),
        label: question.label.to_string(),
        prompt: question.prompt.to_string(),
        expected_answer: question.expected_answer.to_string(),
        grading: question.grading,
        max_runs: question.max_runs,
        timeout_seconds: question.timeout_seconds,
    })
}

fn resolve_custom_answer(grading: ProbeGrading, answer: Option<String>) -> Result<String, String> {
    let answer = answer.unwrap_or_default().trim().to_string();
    if grading != ProbeGrading::Integer {
        return if answer.is_empty() { Ok(answer) }
            else { Err("图像和动画题由人工观察，不接受期望整数答案".to_string()) };
    }
    if answer.is_empty() {
        return Err("自定义题目需要填写期望的整数答案".to_string());
    }
    if !answer.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("期望答案必须是非负整数".to_string());
    }
    Ok(normalize_integer(&answer).to_string())
}

pub(super) fn normalize_integer(value: &str) -> &str {
    let normalized = value.trim_start_matches('0');
    if normalized.is_empty() { "0" } else { normalized }
}

pub(super) fn answer_matches(answer: &str, expected: &str) -> bool {
    let answer = answer.trim();
    !answer.is_empty()
        && answer.bytes().all(|byte| byte.is_ascii_digit())
        && normalize_integer(answer) == expected
}
