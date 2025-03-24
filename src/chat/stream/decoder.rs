use crate::chat::{
    aiserver::v1::{StreamChatResponse, WebReference},
    error::{ChatError, StreamError},
};
use flate2::read::GzDecoder;
use prost::Message;
use std::io::Read;

// 解压gzip数据
fn decompress_gzip(data: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();

    match decoder.read_to_end(&mut decompressed) {
        Ok(_) => Some(decompressed),
        Err(_) => {
            // println!("gzip解压失败: {}", e);
            None
        }
    }
}

pub trait ToMarkdown {
    fn to_markdown(&self) -> String;
}

impl ToMarkdown for Vec<WebReference> {
    fn to_markdown(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let mut result = String::from("WebReferences:\n");
        for (i, web_ref) in self.iter().enumerate() {
            result.push_str(&format!(
                "{}. [{}]({})<{}>\n",
                i + 1,
                web_ref.title,
                web_ref.url,
                web_ref.chunk
            ));
        }
        result.push('\n');
        result
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum StreamMessage {
    // 调试
    Debug(String),
    // 网络引用
    WebReference(Vec<WebReference>),
    // 内容开始标志
    ContentStart,
    // 消息内容
    Content(String),
    // 流结束标志
    StreamEnd,
}

impl StreamMessage {
    fn convert_web_ref_to_content(self) -> Self {
        match self {
            StreamMessage::WebReference(refs) => StreamMessage::Content(refs.to_markdown()),
            other => other,
        }
    }
}

pub struct StreamDecoder {
    buffer: Vec<u8>,
    first_result: Option<Vec<StreamMessage>>,
    first_result_ready: bool,
    first_result_taken: bool,
}

impl StreamDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            first_result: None,
            first_result_ready: false,
            first_result_taken: false,
        }
    }

    pub fn take_first_result(&mut self) -> Option<Vec<StreamMessage>> {
        if !self.buffer.is_empty() {
            return None;
        }
        if self.first_result.is_some() {
            self.first_result_taken = true;
        }
        self.first_result.take()
    }

    #[cfg(test)]
    fn is_incomplete(&self) -> bool {
        !self.buffer.is_empty()
    }

    pub fn is_first_result_ready(&self) -> bool {
        self.first_result_ready
    }

    pub fn decode(
        &mut self,
        data: &[u8],
        convert_web_ref: bool,
    ) -> Result<Vec<StreamMessage>, StreamError> {
        self.buffer.extend_from_slice(data);

        if self.buffer.len() < 5 {
            if self.buffer.is_empty() {
                return Err(StreamError::EmptyStream);
            }
            crate::debug_println!("数据长度小于5字节，当前数据: {}", hex::encode(&self.buffer));
            return Err(StreamError::DataLengthLessThan5);
        }

        let mut messages = Vec::new();
        let mut offset = 0;

        while offset + 5 <= self.buffer.len() {
            let msg_type = self.buffer[offset];
            let msg_len = u32::from_be_bytes([
                self.buffer[offset + 1],
                self.buffer[offset + 2],
                self.buffer[offset + 3],
                self.buffer[offset + 4],
            ]) as usize;

            if msg_len == 0 {
                offset += 5;
                messages.push(StreamMessage::ContentStart);
                continue;
            }

            if offset + 5 + msg_len > self.buffer.len() {
                break;
            }

            let msg_data = &self.buffer[offset + 5..offset + 5 + msg_len];

            if let Some(msg) = self.process_message(msg_type, msg_data)? {
                if convert_web_ref {
                    messages.push(msg.convert_web_ref_to_content());
                } else {
                    messages.push(msg);
                }
            }

            offset += 5 + msg_len;
        }

        self.buffer.drain(..offset);

        if !self.first_result_taken && !messages.is_empty() {
            if self.first_result.is_none() {
                self.first_result = Some(messages.clone());
            } else if !self.first_result_ready {
                self.first_result.as_mut().unwrap().extend(messages.clone());
            }
        }
        if !self.first_result_ready {
            self.first_result_ready =
                self.first_result.is_some() && self.buffer.is_empty() && !self.first_result_taken;
        }
        Ok(messages)
    }

    fn process_message(
        &self,
        msg_type: u8,
        msg_data: &[u8],
    ) -> Result<Option<StreamMessage>, StreamError> {
        match msg_type {
            0 => self.handle_text_message(msg_data),
            1 => self.handle_gzip_message(msg_data),
            2 => self.handle_json_message(msg_data),
            3 => self.handle_gzip_json_message(msg_data),
            t => {
                eprintln!("收到未知消息类型: {}，请尝试联系开发者以获取支持", t);
                crate::debug_println!("消息类型: {}，消息内容: {}", t, hex::encode(msg_data));
                Ok(None)
            }
        }
    }

    fn handle_text_message(&self, msg_data: &[u8]) -> Result<Option<StreamMessage>, StreamError> {
        if let Ok(response) = StreamChatResponse::decode(msg_data) {
            // println!("[text] StreamChatResponse [hex: {}]: {:?}", hex::encode(msg_data), response);
            if !response.text.is_empty() {
                Ok(Some(StreamMessage::Content(response.text)))
            } else if let Some(filled_prompt) = response.filled_prompt {
                Ok(Some(StreamMessage::Debug(filled_prompt)))
            } else if let Some(web_citation) = response.web_citation {
                Ok(Some(StreamMessage::WebReference(web_citation.references)))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn handle_gzip_message(&self, msg_data: &[u8]) -> Result<Option<StreamMessage>, StreamError> {
        if let Some(text) = decompress_gzip(msg_data) {
            if let Ok(response) = StreamChatResponse::decode(&text[..]) {
                // println!("[gzip] StreamChatResponse [hex: {}]: {:?}", hex::encode(msg_data), response);
                if !response.text.is_empty() {
                    Ok(Some(StreamMessage::Content(response.text)))
                } else if let Some(filled_prompt) = response.filled_prompt {
                    Ok(Some(StreamMessage::Debug(filled_prompt)))
                } else if let Some(web_citation) = response.web_citation {
                    Ok(Some(StreamMessage::WebReference(web_citation.references)))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn handle_json_message(&self, msg_data: &[u8]) -> Result<Option<StreamMessage>, StreamError> {
        if msg_data.len() == 2 {
            return Ok(Some(StreamMessage::StreamEnd));
        }
        if let Ok(text) = String::from_utf8(msg_data.to_vec()) {
            // println!("[text] JSON消息 [hex: {}]: {}", hex::encode(msg_data), text);
            if let Ok(error) = serde_json::from_str::<ChatError>(&text) {
                return Err(StreamError::ChatError(error));
            }
        }
        Ok(None)
    }

    fn handle_gzip_json_message(
        &self,
        msg_data: &[u8],
    ) -> Result<Option<StreamMessage>, StreamError> {
        if let Some(text) = decompress_gzip(msg_data) {
            if text.len() == 2 {
                return Ok(Some(StreamMessage::StreamEnd));
            }
            if let Ok(text) = String::from_utf8(text) {
                // println!("[gzip] JSON消息 [hex: {}]: {}", hex::encode(msg_data), text);
                if let Ok(error) = serde_json::from_str::<ChatError>(&text) {
                    return Err(StreamError::ChatError(error));
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_chunk() {
        // 使用include_str!加载测试数据文件
        let stream_data = include_str!("../../../tests/data/stream_data.txt");

        // 将整个字符串按每两个字符分割成字节
        let bytes: Vec<u8> = stream_data
            .as_bytes()
            .chunks(2)
            .map(|chunk| {
                let hex_str = std::str::from_utf8(chunk).unwrap();
                u8::from_str_radix(hex_str, 16).unwrap()
            })
            .collect();

        // 创建解码器
        let mut decoder = StreamDecoder::new();

        match decoder.decode(&bytes, false) {
            Ok(messages) => {
                for message in messages {
                    match message {
                        StreamMessage::StreamEnd => {
                            println!("流结束");
                            break;
                        }
                        StreamMessage::Content(msg) => {
                            println!("消息内容: {}", msg);
                        }
                        StreamMessage::WebReference(refs) => {
                            println!("网页引用:");
                            for (i, web_ref) in refs.iter().enumerate() {
                                println!(
                                    "{}. {} - {} - {}",
                                    i, web_ref.url, web_ref.title, web_ref.chunk
                                );
                            }
                        }
                        StreamMessage::Debug(prompt) => {
                            println!("调试信息: {}", prompt);
                        }
                        StreamMessage::ContentStart => {
                            println!("流开始");
                        }
                    }
                }
            }
            Err(e) => {
                println!("解析错误: {}", e);
            }
        }
        if decoder.is_incomplete() {
            println!("数据不完整");
        }
    }

    #[test]
    fn test_multiple_chunks() {
        // 使用include_str!加载测试数据文件
        let stream_data = include_str!("../../../tests/data/stream_data.txt");

        // 将整个字符串按每两个字符分割成字节
        let bytes: Vec<u8> = stream_data
            .as_bytes()
            .chunks(2)
            .map(|chunk| {
                let hex_str = std::str::from_utf8(chunk).unwrap();
                u8::from_str_radix(hex_str, 16).unwrap()
            })
            .collect();

        // 创建解码器
        let mut decoder = StreamDecoder::new();

        // 辅助函数：找到下一个消息边界
        fn find_next_message_boundary(bytes: &[u8]) -> usize {
            if bytes.len() < 5 {
                return bytes.len();
            }
            let msg_len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
            5 + msg_len
        }

        // 辅助函数：将字节转换为hex字符串
        fn bytes_to_hex(bytes: &[u8]) -> String {
            bytes
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<String>>()
                .join("")
        }

        // 多次解析数据
        let mut offset = 0;
        let mut should_break = false;

        while offset < bytes.len() {
            let remaining_bytes = &bytes[offset..];
            let msg_boundary = find_next_message_boundary(remaining_bytes);
            let current_msg_bytes = &remaining_bytes[..msg_boundary];
            let hex_str = bytes_to_hex(current_msg_bytes);

            match decoder.decode(current_msg_bytes, false) {
                Ok(messages) => {
                    for message in messages {
                        match message {
                            StreamMessage::StreamEnd => {
                                println!("流结束 [hex: {}]", hex_str);
                                should_break = true;
                                break;
                            }
                            StreamMessage::Content(msg) => {
                                println!("消息内容 [hex: {}]: {}", hex_str, msg);
                            }
                            StreamMessage::WebReference(refs) => {
                                println!("网页引用 [hex: {}]:", hex_str);
                                for (i, web_ref) in refs.iter().enumerate() {
                                    println!(
                                        "{}. {} - {} - {}",
                                        i, web_ref.url, web_ref.title, web_ref.chunk
                                    );
                                }
                            }
                            StreamMessage::Debug(prompt) => {
                                println!("调试信息 [hex: {}]: {}", hex_str, prompt);
                            }
                            StreamMessage::ContentStart => {
                                println!("流开始 [hex: {}]", hex_str);
                            }
                        }
                    }
                    if should_break {
                        break;
                    }
                    if decoder.is_incomplete() {
                        println!("数据不完整 [hex: {}]", hex_str);
                        break;
                    }
                    offset += msg_boundary;
                }
                Err(e) => {
                    println!("解析错误 [hex: {}]: {}", hex_str, e);
                    break;
                }
            }
        }
    }
}
