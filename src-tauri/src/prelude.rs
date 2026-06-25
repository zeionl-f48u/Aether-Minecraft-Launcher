//! 预导入模块
//!
//! 提供各模块常用的类型和 trait 别名。
//! 各模块通过 `use crate::prelude::*;` 导入。
//! 各模块通过 `use crate::prelude::*;` 导入。

// 重新导出标准库常用 trait
pub use std::fmt;
pub use std::path::{Path, PathBuf};

// 重新导出异步运行时
pub use tokio;

// 重新导出序列化库
pub use serde::{Deserialize, Serialize};
pub use serde_json;

// 重新导出 HTTP 客户端
pub use crate::components::http::HttpClient;

// 重新导出路径管理
pub use crate::components::path::MinecraftPaths;

// 重新导出进度报告
pub use crate::components::progress::{NoopReporter, Reporter, ReportState};

// 重新导出常用错误转换
pub use thiserror::Error as ThisError;
