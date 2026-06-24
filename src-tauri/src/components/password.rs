//! 安全的密码包装类型
//!
//! 所有格式化输出（Display、Debug）都会显示为 `***Password***`。

use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

// ============================================================================
//  密码类型
// ============================================================================

/// 密码包装类型，防止敏感信息在日志中泄露
///
/// # 示例
/// ```
/// use scl_core::password::Password;
/// let password = Password::from("my_secret");
/// println!("{}", password); // 输出: ***Password***
/// ```
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Password(String);

impl Password {
    /// 创建一个新的密码实例
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// 获取密码的字符串切片（明文）
    ///
    /// # 安全警告
    /// 请确保在安全上下文中使用，避免泄露。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 获取密码的字符串（克隆）
    ///
    /// # 安全警告
    /// 请确保在安全上下文中使用，避免泄露。
    pub fn to_string(&self) -> String {
        self.0.clone()
    }

    /// 获取密码的字符串（消耗自身）
    ///
    /// # 安全警告
    /// 请确保在安全上下文中使用，避免泄露。
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }

    /// 检查密码是否为空
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 获取密码长度（注意：暴露长度信息可能也是安全风险）
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

// ============================================================================
//  格式化输出（安全掩码）
// ============================================================================

impl Debug for Password {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("***Password***")
    }
}

impl Display for Password {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("***Password***")
    }
}

// ============================================================================
//  序列化与反序列化
// ============================================================================

impl Serialize for Password {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 序列化时使用明文（配置文件存储需要）
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Password {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PasswordVisitor;
        impl<'de> serde::de::Visitor<'de> for PasswordVisitor {
            type Value = Password;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a password string")
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Password(v.to_string()))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Password(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Password(v))
            }
        }
        deserializer.deserialize_str(PasswordVisitor)
    }
}

// ============================================================================
//  类型转换
// ============================================================================

impl From<String> for Password {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Password {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<Password> for String {
    fn from(p: Password) -> Self {
        p.0
    }
}

impl From<&Password> for String {
    fn from(p: &Password) -> Self {
        p.0.clone()
    }
}

impl AsRef<str> for Password {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ============================================================================
//  移除 Deref 实现（已注释掉，改为显式方法）
// ============================================================================

// 注意：不实现 Deref，以防止意外暴露内部字符串。
// 如需访问密码，请使用 as_str()、to_string() 或 into_string()。

// ============================================================================
//  可选：Zeroizing 清理（需要 zeroize crate）
// ============================================================================

// 如果启用 zeroize 功能，可以在 Drop 时清零内存
// 但 Zeroizing 要求类型实现 Zeroize，对 String 的支持需要额外处理。
// 暂不启用，保持简单。

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_creation() {
        let p = Password::new("secret");
        assert_eq!(p.as_str(), "secret");
        assert_eq!(p.to_string(), "secret");
    }

    #[test]
    fn test_password_display() {
        let p = Password::from("secret");
        assert_eq!(format!("{}", p), "***Password***");
        assert_eq!(format!("{:?}", p), "***Password***");
    }

    #[test]
    fn test_password_serialize() {
        let p = Password::from("secret");
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "\"secret\"");
    }

    #[test]
    fn test_password_deserialize() {
        let json = "\"secret\"";
        let p: Password = serde_json::from_str(json).unwrap();
        assert_eq!(p.as_str(), "secret");
    }

    #[test]
    fn test_password_conversion() {
        let p = Password::from("secret");
        let s1: String = p.clone().into();
        let s2: String = (&p).into();
        assert_eq!(s1, "secret");
        assert_eq!(s2, "secret");

        let p2 = Password::from(s1);
        assert_eq!(p2.as_str(), "secret");
    }

    #[test]
    fn test_password_len() {
        let p = Password::from("secret");
        assert_eq!(p.len(), 6);
        assert!(!p.is_empty());

        let empty = Password::default();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_password_as_ref() {
        let p = Password::from("secret");
        assert_eq!(p.as_ref(), "secret");
    }
}