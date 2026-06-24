//! 解析 Minecraft 版本号
//!
//! 支持传统的语义化版本（如 1.16.5、1.21.4）和自 2026 年起启用的新年度版本号系统（如 26.1、26.1.2）。

use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
    str::FromStr,
};

use thiserror::Error;
use lazy_static::lazy_static;
use regex::Regex;

// ============================================================================
//  错误处理增强
// ============================================================================

/// 版本号解析错误类型
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseVersionError {
    #[error("无效的版本号格式: '{0}'")]
    InvalidFormat(String),
    #[error("版本号数字解析失败: '{0}'")]
    InvalidNumber(String),
}

// ============================================================================
//  Minecraft 版本号枚举
// ============================================================================

/// Minecraft 版本号
///
/// 支持传统格式（1.x）和新年度格式（YY.D.P），以及快照和自定义版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinecraftVersion {
    /// 传统正式版本，如 1.16.5、1.21.4
    Release(u32, u32, u32),
    /// 年度版本，如 26.1（第一个发布）、26.1.2（补丁）
    Yearly(u32, u32, Option<u32>),
    /// 快照版本，如 25w41a（旧格式）、26.1-snapshot-2（新格式）
    Snapshot(String),
    /// 自定义特殊版本（Beta、Alpha 等）
    Custom(String),
}

impl Default for MinecraftVersion {
    fn default() -> Self {
        Self::Custom(String::new())
    }
}

// ============================================================================
//  字符串解析（FromStr）
// ============================================================================

impl FromStr for MinecraftVersion {
    type Err = ParseVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_version(s).ok_or_else(|| ParseVersionError::InvalidFormat(s.to_string()))
    }
}

// ============================================================================
//  显示（Display）
// ============================================================================

impl Display for MinecraftVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MinecraftVersion::Release(a, b, c) => {
                if *c == 0 {
                    write!(f, "{a}.{b}")
                } else {
                    write!(f, "{a}.{b}.{c}")
                }
            }
            MinecraftVersion::Yearly(year, drop, patch) => {
                if let Some(p) = patch {
                    write!(f, "{year}.{drop}.{p}")
                } else {
                    write!(f, "{year}.{drop}")
                }
            }
            MinecraftVersion::Snapshot(s) => write!(f, "{s}"),
            MinecraftVersion::Custom(c) => write!(f, "{c}"),
        }
    }
}

// ============================================================================
//  版本比较（手动实现 Ord）
// ============================================================================

impl PartialOrd for MinecraftVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MinecraftVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        use MinecraftVersion::*;

        // 定义版本类型的优先级顺序（从旧到新）
        // 注意：这只是个示例，实际顺序可能需要根据业务调整
        fn version_priority(v: &MinecraftVersion) -> u8 {
            match v {
                Custom(_) => 0,
                Release(_, _, _) => 1,
                Snapshot(_) => 2,
                Yearly(_, _, _) => 3,
            }
        }

        let priority_order = version_priority(self).cmp(&version_priority(other));

        if priority_order != Ordering::Equal {
            return priority_order;
        }

        match (self, other) {
            (Release(a1, b1, c1), Release(a2, b2, c2)) => {
                a1.cmp(a2).then(b1.cmp(b2)).then(c1.cmp(c2))
            }
            (Yearly(y1, d1, p1), Yearly(y2, d2, p2)) => {
                y1.cmp(y2)
                    .then(d1.cmp(d2))
                    .then(p1.unwrap_or(0).cmp(&p2.unwrap_or(0)))
            }
            (Snapshot(s1), Snapshot(s2)) => s1.cmp(s2),
            (Custom(c1), Custom(c2)) => c1.cmp(c2),
            _ => Ordering::Equal, // 不应发生
        }
    }
}

// ============================================================================
//  版本解析函数（核心）
// ============================================================================

lazy_static! {
    /// 传统版本格式正则：1.16.5 或 1.8
    static ref RELEASE_RE: Regex = Regex::new(r"^(\d+)\.(\d+)(?:\.(\d+))?$").unwrap();
    /// 年度版本格式正则：26.1 或 26.1.2
    static ref YEARLY_RE: Regex = Regex::new(r"^(\d{2})\.(\d+)(?:\.(\d+))?$").unwrap();
    /// 旧快照格式正则：25w41a
    static ref SNAPSHOT_OLD_RE: Regex = Regex::new(r"^(\d{2})w(\d{2})([a-z])$").unwrap();
    /// 新快照格式正则：26.1-snapshot-2
    static ref SNAPSHOT_NEW_RE: Regex = Regex::new(r"^(\d{2})\.(\d+)-snapshot-(\d+)$").unwrap();
}

/// 解析版本字符串为 MinecraftVersion
///
/// 支持以下格式：
/// - 传统正式版：`1.16.5`, `1.8`
/// - 年度版本：`26.1`, `26.1.2`
/// - 旧快照：`25w41a`
/// - 新快照：`26.1-snapshot-2`
/// - 自定义：其他任何字符串
pub fn parse_version(input: &str) -> Option<MinecraftVersion> {
    let input = input.trim();

    // 1. 尝试解析传统正式版
    if let Some(caps) = RELEASE_RE.captures(input) {
        let major: u32 = caps[1].parse().ok()?;
        let minor: u32 = caps[2].parse().ok()?;
        let patch: u32 = caps.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        return Some(MinecraftVersion::Release(major, minor, patch));
    }

    // 2. 尝试解析年度版本
    if let Some(caps) = YEARLY_RE.captures(input) {
        let year: u32 = caps[1].parse().ok()?;
        let drop: u32 = caps[2].parse().ok()?;
        let patch: Option<u32> = caps.get(3).map(|m| m.as_str().parse().ok()).flatten();
        // 年份必须是 26 或更大（2026年及以后）
        if year >= 26 {
            return Some(MinecraftVersion::Yearly(year, drop, patch));
        }
        // 如果年份小于 26，但格式匹配，可能是误匹配，继续尝试其他格式
    }

    // 3. 尝试解析旧快照格式
    if let Some(caps) = SNAPSHOT_OLD_RE.captures(input) {
        let year: u32 = caps[1].parse().ok()?;
        let week: u32 = caps[2].parse().ok()?;
        let letter = caps[3].to_string();
        // 旧快照格式：年份 + "w" + 周数 + 字母
        if year >= 10 && year <= 25 && week >= 1 && week <= 52 {
            return Some(MinecraftVersion::Snapshot(format!("{:02}w{:02}{}", year, week, letter)));
        }
        // 如果年份不在合理范围，继续尝试其他格式
    }

    // 4. 尝试解析新快照格式
    if let Some(caps) = SNAPSHOT_NEW_RE.captures(input) {
        let year: u32 = caps[1].parse().ok()?;
        let drop: u32 = caps[2].parse().ok()?;
        let snapshot_num: u32 = caps[3].parse().ok()?;
        if year >= 26 {
            return Some(MinecraftVersion::Snapshot(format!("{}.{}-snapshot-{}", year, drop, snapshot_num)));
        }
    }

    // 5. 都不匹配，作为自定义版本
    Some(MinecraftVersion::Custom(input.to_string()))
}

// ============================================================================
//  Java 版本要求（集中管理）
// ============================================================================

/// Java 版本要求配置
///
/// 将版本到 Java 版本的映射集中管理，便于维护和更新
struct JavaRequirement {
    /// 匹配的版本范围（闭区间）
    range: (MinecraftVersion, MinecraftVersion),
    /// 所需的 Java 主版本
    java_version: u8,
}

/// 获取 Java 版本要求映射表
///
/// 将硬编码的版本判断集中在此，便于未来更新
fn get_java_requirements() -> Vec<JavaRequirement> {
    vec![
        // 1.21+ 需要 Java 21
        JavaRequirement {
            range: (
                MinecraftVersion::Release(1, 21, 0),
                MinecraftVersion::Release(1, u32::MAX, u32::MAX),
            ),
            java_version: 21,
        },
        // 1.17 - 1.20 需要 Java 16
        JavaRequirement {
            range: (
                MinecraftVersion::Release(1, 17, 0),
                MinecraftVersion::Release(1, 20, u32::MAX),
            ),
            java_version: 16,
        },
        // 旧版本需要 Java 8
        JavaRequirement {
            range: (
                MinecraftVersion::Release(0, 0, 0),
                MinecraftVersion::Release(1, 16, u32::MAX),
            ),
            java_version: 8,
        },
    ]
}

// ============================================================================
//  MinecraftVersion 方法实现
// ============================================================================

impl MinecraftVersion {
    /// 检查该版本需要的最低 Java 版本
    pub fn required_java_version(&self) -> u8 {
        // 处理年度版本：假设 26.x 需要 Java 21
        if let Self::Yearly(year, _, _) = self {
            if year >= 26 {
                return 21;
            }
        }

        // 处理快照版本
        if let Self::Snapshot(s) = self {
            // 尝试从快照名称中提取年份和版本信息
            // 旧快照格式：25w41a -> 2025年
            if let Some(caps) = SNAPSHOT_OLD_RE.captures(s) {
                if let Ok(year) = caps[1].parse::<u32>() {
                    // 2021年及以后的快照需要 Java 16
                    if year >= 21 {
                        return 16;
                    }
                }
            }
            // 新快照格式：26.1-snapshot-2 -> 属于 26.x 系列
            if let Some(caps) = SNAPSHOT_NEW_RE.captures(s) {
                if let Ok(year) = caps[1].parse::<u32>() {
                    if year >= 26 {
                        return 21;
                    }
                }
            }
            // 其他快照默认 Java 8
            return 8;
        }

        // 传统正式版：使用配置表
        if let Self::Release(major, minor, patch) = self {
            for req in get_java_requirements() {
                let (start, end) = req.range;
                if let (MinecraftVersion::Release(s_maj, s_min, s_patch),
                       MinecraftVersion::Release(e_maj, e_min, e_patch)) = (start, end) {
                    if (*major, *minor, *patch) >= (s_maj, s_min, s_patch) &&
                       (*major, *minor, *patch) <= (e_maj, e_min, e_patch) {
                        return req.java_version;
                    }
                }
            }
        }

        // 默认 Java 8
        8
    }

    /// 判断 Forge 是否使用覆盖安装方式
    ///
    /// 1.5.1 及更早版本返回 true
    pub fn should_forge_use_override_installation(&self) -> bool {
        match self {
            MinecraftVersion::Release(a, b, c) => {
                match a.cmp(&1) {
                    Ordering::Greater => false,
                    Ordering::Equal => match b.cmp(&1) {
                        Ordering::Greater => false,
                        Ordering::Equal => *c < 2,
                        Ordering::Less => true,
                    },
                    Ordering::Less => true,
                }
            }
            // 年度版本和快照默认使用安装器
            _ => false,
        }
    }

    /// 判断 Forge 下载文件名后缀
    ///
    /// 1.2.5 及更早版本返回 true（使用 client），否则使用 universal
    pub fn should_forge_use_client_or_universal(&self) -> bool {
        match self {
            MinecraftVersion::Release(a, b, c) => {
                match a.cmp(&1) {
                    Ordering::Greater => false,
                    Ordering::Equal => match b.cmp(&2) {
                        Ordering::Greater => false,
                        Ordering::Equal => *c <= 5,
                        Ordering::Less => true,
                    },
                    Ordering::Less => true,
                }
            }
            // 年度版本和快照使用 universal
            _ => false,
        }
    }

    /// 判断是否为年度版本（2026年及以后）
    pub fn is_yearly_version(&self) -> bool {
        matches!(self, MinecraftVersion::Yearly(_, _, _))
    }

    /// 判断是否为快照版本
    pub fn is_snapshot(&self) -> bool {
        matches!(self, MinecraftVersion::Snapshot(_))
    }

    /// 获取版本年份（如果可提取）
    pub fn year(&self) -> Option<u32> {
        match self {
            MinecraftVersion::Yearly(year, _, _) => Some(*year),
            MinecraftVersion::Snapshot(s) => {
                // 尝试从快照名称中提取年份
                if let Some(caps) = SNAPSHOT_OLD_RE.captures(s) {
                    caps[1].parse().ok()
                } else if let Some(caps) = SNAPSHOT_NEW_RE.captures(s) {
                    caps[1].parse().ok()
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

// ============================================================================
//  单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_release() {
        assert_eq!(
            parse_version("1.16.5").unwrap(),
            MinecraftVersion::Release(1, 16, 5)
        );
        assert_eq!(
            parse_version("1.8").unwrap(),
            MinecraftVersion::Release(1, 8, 0)
        );
        assert_eq!(
            parse_version("1.21.4").unwrap(),
            MinecraftVersion::Release(1, 21, 4)
        );
    }

    #[test]
    fn test_parse_yearly() {
        assert_eq!(
            parse_version("26.1").unwrap(),
            MinecraftVersion::Yearly(26, 1, None)
        );
        assert_eq!(
            parse_version("26.1.2").unwrap(),
            MinecraftVersion::Yearly(26, 1, Some(2))
        );
        assert_eq!(
            parse_version("27.5").unwrap(),
            MinecraftVersion::Yearly(27, 5, None)
        );
    }

    #[test]
    fn test_parse_snapshot_old() {
        assert_eq!(
            parse_version("25w41a").unwrap(),
            MinecraftVersion::Snapshot("25w41a".to_string())
        );
        assert_eq!(
            parse_version("21w08b").unwrap(),
            MinecraftVersion::Snapshot("21w08b".to_string())
        );
    }

    #[test]
    fn test_parse_snapshot_new() {
        assert_eq!(
            parse_version("26.1-snapshot-2").unwrap(),
            MinecraftVersion::Snapshot("26.1-snapshot-2".to_string())
        );
        assert_eq!(
            parse_version("26.2-snapshot-1").unwrap(),
            MinecraftVersion::Snapshot("26.2-snapshot-1".to_string())
        );
    }

    #[test]
    fn test_parse_custom() {
        assert_eq!(
            parse_version("beta_1.7.3").unwrap(),
            MinecraftVersion::Custom("beta_1.7.3".to_string())
        );
        assert_eq!(
            parse_version("1.7.10-forge").unwrap(),
            MinecraftVersion::Custom("1.7.10-forge".to_string())
        );
    }

    #[test]
    fn test_display() {
        assert_eq!(&MinecraftVersion::Release(1, 16, 5).to_string(), "1.16.5");
        assert_eq!(&MinecraftVersion::Release(1, 8, 0).to_string(), "1.8");
        assert_eq!(&MinecraftVersion::Yearly(26, 1, None).to_string(), "26.1");
        assert_eq!(&MinecraftVersion::Yearly(26, 1, Some(2)).to_string(), "26.1.2");
        assert_eq!(
            &MinecraftVersion::Snapshot("25w41a".to_string()).to_string(),
            "25w41a"
        );
        assert_eq!(
            &MinecraftVersion::Snapshot("26.1-snapshot-2".to_string()).to_string(),
            "26.1-snapshot-2"
        );
    }

    #[test]
    fn test_required_java_version() {
        // 传统版本
        assert_eq!(MinecraftVersion::Release(1, 16, 5).required_java_version(), 8);
        assert_eq!(MinecraftVersion::Release(1, 17, 0).required_java_version(), 16);
        assert_eq!(MinecraftVersion::Release(1, 20, 0).required_java_version(), 16);
        assert_eq!(MinecraftVersion::Release(1, 21, 0).required_java_version(), 21);

        // 年度版本
        assert_eq!(MinecraftVersion::Yearly(26, 1, None).required_java_version(), 21);

        // 快照
        assert_eq!(
            MinecraftVersion::Snapshot("21w08b".to_string()).required_java_version(),
            16
        );
        assert_eq!(
            MinecraftVersion::Snapshot("26.1-snapshot-2".to_string()).required_java_version(),
            21
        );
    }

    #[test]
    fn test_ordering() {
        let v1 = MinecraftVersion::Release(1, 16, 5);
        let v2 = MinecraftVersion::Release(1, 17, 0);
        assert!(v1 < v2);

        let v3 = MinecraftVersion::Yearly(26, 1, None);
        let v4 = MinecraftVersion::Yearly(26, 2, None);
        assert!(v3 < v4);

        let v5 = MinecraftVersion::Yearly(26, 1, None);
        let v6 = MinecraftVersion::Yearly(26, 1, Some(1));
        assert!(v5 < v6);
    }

    #[test]
    fn test_forge_methods() {
        // 覆盖安装测试
        assert!(MinecraftVersion::Release(1, 5, 1).should_forge_use_override_installation());
        assert!(!MinecraftVersion::Release(1, 6, 0).should_forge_use_override_installation());
        assert!(!MinecraftVersion::Yearly(26, 1, None).should_forge_use_override_installation());

        // client/universal 测试
        assert!(MinecraftVersion::Release(1, 2, 5).should_forge_use_client_or_universal());
        assert!(!MinecraftVersion::Release(1, 3, 0).should_forge_use_client_or_universal());
        assert!(!MinecraftVersion::Yearly(26, 1, None).should_forge_use_client_or_universal());
    }
}