//! 后端业务逻辑服务层。
//!
//! 各模块职责:
//! - [`csv_reader`] — CSV 解析、列映射、多文件拼接
//! - [`downsampling`] — LTTB 降采样算法（索引选择）
//! - [`project_file`] — `.vibproj` 项目文件的保存与加载 (ZIP + Parquet)
//! - [`project_scanner`] — AIDPS 文件夹结构扫描与 channel schema 检测
//! - [`stats_engine`] — 振动数据统计计算（基本/分布/形状统计）
//! - [`time_filter`] — Polars 时间范围过滤工具

pub mod csv_reader;
pub mod downsampling;
pub mod project_file;
pub mod project_scanner;
pub mod stats_engine;
pub mod time_filter;
