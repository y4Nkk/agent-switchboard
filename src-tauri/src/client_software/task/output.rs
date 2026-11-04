use crate::{client_software::{process::Chunk, updater, types::UpdateTask}, outbound_proxy::OutboundProxySettings};

#[derive(Default)]
pub struct Output {
    pending: [Vec<u8>; 2],
    dropping: [bool; 2],
}

impl Output {
    pub fn push(&mut self, chunk: Chunk, task: &mut UpdateTask, proxy: &OutboundProxySettings) {
        let index = usize::from(chunk.stderr);
        for byte in chunk.bytes {
            if byte == b'\n' || byte == b'\r' {
                self.line(index, task, proxy);
                self.dropping[index] = false;
            } else if !self.dropping[index] {
                self.pending[index].push(byte);
                if self.pending[index].len() > 16_384 {
                    self.pending[index].clear();
                    self.dropping[index] = true;
                    updater::log(task, "[oversized output line omitted]", proxy);
                }
            }
        }
    }

    pub fn finish(&mut self, task: &mut UpdateTask, proxy: &OutboundProxySettings) {
        for index in 0..2 { self.line(index, task, proxy); }
    }

    fn line(&mut self, index: usize, task: &mut UpdateTask, proxy: &OutboundProxySettings) {
        if self.pending[index].is_empty() { return; }
        let text = String::from_utf8_lossy(&self.pending[index]);
        updater::log(task, &text, proxy);
        self.pending[index].clear();
    }
}
