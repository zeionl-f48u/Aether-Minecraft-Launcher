//! 解析 Maven 包名称（group:artifact:version[:classifier]）
//!
//! 用于生成 Maven 仓库中的 JAR 文件下载路径。

use std::fmt::{Display, Formatter};
use std::str::FromStr;
use thiserror::Error;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParsePackageError {
    #[error("无效的包格式，需要 'group:artifact:version' 或 'group:artifact:version:classifier'")]
    InvalidFormat,
    #[error("包名或版本不能为空")]
    EmptyComponent,
}

pub type ParsePackageResult<T> = Result<T, ParsePackageError>;

// ============================================================================
//  包名结构
// ============================================================================

/// Maven 包名，包含 group、artifact、version 和可选的 classifier。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageName {
    /// group 路径，以点分隔，如 "net.fabricmc"
    pub namespaces: Vec<String>,
    /// artifact 名称
    pub name: String,
    /// 版本号
    pub version: String,
    /// 可选 classifier（如 "sources", "javadoc"）
    pub classifier: Option<String>,
}

impl PackageName {
    /// 创建新的包名。
    pub fn new(
        namespaces: Vec<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        classifier: Option<String>,
    ) -> Self {
        Self {
            namespaces,
            name: name.into(),
            version: version.into(),
            classifier,
        }
    }

    /// 生成 Maven 仓库中 JAR 文件的相对路径或 URL。
    ///
    /// # 参数
    /// - `base_url_or_path`: Maven 仓库的基础 URL 或本地路径，如 "https://maven.fabricmc.net"。
    ///   如果末尾没有斜杠，会自动添加。
    ///
    /// # 示例
    /// ```
    /// let pkg = PackageName::from_str("net.fabricmc:fabric-loader:0.11.3").unwrap();
    /// assert_eq!(
    ///     pkg.to_maven_jar_path("https://maven.fabricmc.net"),
    ///     "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.11.3/fabric-loader-0.11.3.jar"
    /// );
    /// ```
    pub fn to_maven_jar_path(&self, base_url_or_path: &str) -> String {
        let base = base_url_or_path.trim_end_matches('/');
        let classifier_part = self
            .classifier
            .as_ref()
            .map(|c| format!("-{}", c))
            .unwrap_or_default();
        format!(
            "{}/{}/{}/{}/{}-{}{}.jar",
            base,
            self.namespaces.join("/"),
            self.name,
            self.version,
            self.name,
            self.version,
            classifier_part
        )
    }
}

// ============================================================================
//  解析实现
// ============================================================================

impl FromStr for PackageName {
    type Err = ParsePackageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_package_name(s)
    }
}

impl Display for PackageName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.namespaces.join("."),
            self.name,
            self.version
        )?;
        if let Some(c) = &self.classifier {
            write!(f, ":{}", c)?;
        }
        Ok(())
    }
}

/// 解析包名字符串。
///
/// 支持格式：
/// - `group:artifact:version`
/// - `group:artifact:version:classifier`
///
/// # 错误
/// 如果格式无效，返回 `ParsePackageError`。
pub fn parse_package_name(s: &str) -> ParsePackageResult<PackageName> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 3 || parts.len() > 4 {
        return Err(ParsePackageError::InvalidFormat);
    }

    let group_part = parts[0];
    let artifact = parts[1].trim();
    let version = parts[2].trim();
    let classifier = if parts.len() == 4 {
        Some(parts[3].trim().to_string())
    } else {
        None
    };

    if group_part.is_empty() || artifact.is_empty() || version.is_empty() {
        return Err(ParsePackageError::EmptyComponent);
    }

    let namespaces: Vec<String> = group_part.split('.').map(|s| s.to_string()).collect();
    Ok(PackageName {
        namespaces,
        name: artifact.to_string(),
        version: version.to_string(),
        classifier,
    })
}

// ============================================================================
//  向后兼容的 From<&str>（已弃用）
// ============================================================================

impl From<&str> for PackageName {
    fn from(s: &str) -> Self {
        parse_package_name(s).unwrap()
    }
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let pkg = parse_package_name("net.fabricmc:sponge-mixin:0.9.2+mixin.0.8.2").unwrap();
        assert_eq!(pkg.namespaces, vec!["net", "fabricmc"]);
        assert_eq!(pkg.name, "sponge-mixin");
        assert_eq!(pkg.version, "0.9.2+mixin.0.8.2");
        assert_eq!(pkg.classifier, None);
        assert_eq!(
            pkg.to_maven_jar_path("https://maven.fabricmc.net"),
            "https://maven.fabricmc.net/net/fabricmc/sponge-mixin/0.9.2+mixin.0.8.2/sponge-mixin-0.9.2+mixin.0.8.2.jar"
        );
    }

    #[test]
    fn test_parse_with_classifier() {
        let pkg = parse_package_name("org.ow2.asm:asm:9.1:sources").unwrap();
        assert_eq!(pkg.classifier, Some("sources".to_string()));
        assert_eq!(
            pkg.to_maven_jar_path("https://repo1.maven.org/maven2"),
            "https://repo1.maven.org/maven2/org/ow2/asm/asm/9.1/asm-9.1-sources.jar"
        );
    }

    #[test]
    fn test_display() {
        let pkg = PackageName::new(vec!["net".to_string(), "fabricmc".to_string()], "fabric-loader", "0.11.3", None);
        assert_eq!(pkg.to_string(), "net.fabricmc:fabric-loader:0.11.3");

        let pkg2 = PackageName::new(vec!["org".to_string()], "slf4j", "1.7.30", Some("javadoc".to_string()));
        assert_eq!(pkg2.to_string(), "org:slf4j:1.7.30:javadoc");
    }

    #[test]
    fn test_parse_errors() {
        assert_eq!(parse_package_name("invalid"), Err(ParsePackageError::InvalidFormat));
        assert_eq!(parse_package_name("a:b:"), Err(ParsePackageError::EmptyComponent));
        assert_eq!(parse_package_name("a::c"), Err(ParsePackageError::EmptyComponent));
        assert_eq!(parse_package_name(":b:c"), Err(ParsePackageError::EmptyComponent));
    }

    #[test]
    fn test_from_str() {
        let pkg: PackageName = "net.fabricmc:tiny-remapper:0.3.0.70".parse().unwrap();
        assert_eq!(pkg.name, "tiny-remapper");
        assert_eq!(pkg.version, "0.3.0.70");
    }

    #[test]
    fn test_path_with_trailing_slash() {
        let pkg = PackageName::from_str("com.google:gson:2.8.9").unwrap();
        let path = pkg.to_maven_jar_path("https://maven.google.com/");
        assert_eq!(
            path,
            "https://maven.google.com/com/google/gson/2.8.9/gson-2.8.9.jar"
        );
    }
}