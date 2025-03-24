use reqwest::{Client, Proxy};
use serde::{Deserialize, Deserializer};
use serde::{Serialize, Serializer};
// use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

use crate::app::constant::COMMA_STRING;

#[derive(Clone, Default, PartialEq)]
pub enum Proxies {
    No,
    #[default]
    System,
    List(Vec<String>),
}

impl Serialize for Proxies {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Proxies::No => serializer.serialize_str(""),
            Proxies::System => serializer.serialize_str("system"),
            Proxies::List(urls) => serializer.serialize_str(&urls.join(COMMA_STRING)),
        }
    }
}

impl<'de> Deserialize<'de> for Proxies {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Proxies::from_str(&s))
    }
}

impl Proxies {
    /// 从字符串创建 Proxies
    ///
    /// # Arguments
    /// * `s` - 代理字符串:
    ///   - "" 或 "no": 不使用代理
    ///   - "system": 使用系统代理
    ///   - 其他: 尝试解析为代理列表，无效则返回 System
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "" | "no" => Self::No,
            "system" => Self::System,
            urls => {
                let valid_proxies: Vec<String> = urls
                    .split(',')
                    .filter_map(|url| {
                        let trimmed = url.trim();
                        (!trimmed.is_empty() && Proxy::all(trimmed).is_ok())
                            .then(|| trimmed.to_string())
                    })
                    .collect();

                if valid_proxies.is_empty() {
                    Self::default()
                } else {
                    Self::List(valid_proxies)
                }
            }
        }
    }

    pub fn get_client(&self) -> Client {
        match self {
            Proxies::No => Client::builder().no_proxy().build().unwrap(),
            Proxies::System => Client::new(),
            Proxies::List(list) => {
                // 使用第一个代理（已经确保是有效的）
                let proxy = Proxy::all(list[0].clone()).unwrap();
                Client::builder().proxy(proxy).build().unwrap()
            }
        }
    }
}
