use crate::server::errors::{AppError, AppResult};
use error_stack::{ResultExt, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{error, info};

/// 钩子步骤枚举：支持多种操作格式
/// 既支持 key-value 形式（如 {run: "..."}），也支持纯字符串（如 "rm"）
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "HookStepDef", into = "HookStepDef")]
pub enum HookStep {
    /// 执行命令格式：{run: "command"}
    Run { run: String },
    /// 移动文件格式：{mv: "target_dir"}
    Move { mv: String },
    /// WebHook 格式：{webhook: "https://example.com/hook"}
    Webhook { webhook: String },
    /// 删除文件格式："rm"
    Remove(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum HookStepDef {
    Run {
        run: String,
    },
    Move {
        mv: String,
    },
    Webhook {
        webhook: String,
    },
    CommandValue {
        cmd: String,
        #[serde(default)]
        value: Option<String>,
    },
    Remove(String),
}

impl TryFrom<HookStepDef> for HookStep {
    type Error = String;

    fn try_from(value: HookStepDef) -> Result<Self, Self::Error> {
        match value {
            HookStepDef::Run { run } => Ok(HookStep::Run { run }),
            HookStepDef::Move { mv } => Ok(HookStep::Move { mv }),
            HookStepDef::Webhook { webhook } => Ok(HookStep::Webhook { webhook }),
            HookStepDef::Remove(cmd) => Ok(HookStep::Remove(cmd)),
            HookStepDef::CommandValue { cmd, value } => match cmd.as_str() {
                "run" => Ok(HookStep::Run {
                    run: value.ok_or_else(|| "missing value for run hook".to_string())?,
                }),
                "mv" => Ok(HookStep::Move {
                    mv: value.ok_or_else(|| "missing value for mv hook".to_string())?,
                }),
                "webhook" => Ok(HookStep::Webhook {
                    webhook: value.ok_or_else(|| "missing value for webhook hook".to_string())?,
                }),
                "rm" => Ok(HookStep::Remove("rm".to_string())),
                other => Err(format!("unknown hook command: {other}")),
            },
        }
    }
}

impl From<HookStep> for HookStepDef {
    fn from(value: HookStep) -> Self {
        match value {
            HookStep::Run { run } => HookStepDef::Run { run },
            HookStep::Move { mv } => HookStepDef::Move { mv },
            HookStep::Webhook { webhook } => HookStepDef::Webhook { webhook },
            HookStep::Remove(cmd) => HookStepDef::Remove(cmd),
        }
    }
}

impl HookStep {
    /// 执行钩子步骤操作
    ///
    /// # 参数
    /// * `video_paths` - 视频文件路径列表
    ///
    /// # 返回
    /// 执行成功返回Ok(())，失败返回错误信息
    pub async fn execute(&self, video_paths: &[&Path]) -> AppResult<()> {
        match self {
            HookStep::Run { run } => {
                // 执行自定义命令
                let paths = Self::join_video_paths(video_paths);
                self.execute_command(run, paths.as_bytes()).await?;
            }
            HookStep::Move { mv } => {
                // 移动文件到指定目录
                self.move_file(video_paths, mv).await?;
            }
            HookStep::Webhook { webhook } => {
                let paths = Self::join_video_paths(video_paths);
                self.execute_webhook(webhook, paths.as_bytes()).await?;
            }
            HookStep::Remove(cmd) if cmd == "rm" => {
                // 删除文件
                HookStep::remove_file(video_paths).await?;
            }
            HookStep::Remove(cmd) => {
                // 未知命令，返回错误
                bail!(AppError::Custom(format!("Unknown command: {}", cmd)));
            }
        }
        Ok(())
    }

    /// 执行钩子步骤操作
    ///
    /// # 参数
    /// * `video_paths` - 视频文件路径列表
    ///
    /// # 返回
    /// 执行成功返回Ok(())，失败返回错误信息
    pub async fn execute_with(&self, src: &[u8]) -> AppResult<()> {
        match self {
            HookStep::Run { run } => {
                self.execute_command(run, src).await?;
            }
            HookStep::Webhook { webhook } => {
                self.execute_webhook(webhook, src).await?;
            }
            cmd => {
                // 未知命令，返回错误
                bail!(AppError::Custom(format!("不支持的命令: {:?}", cmd)));
            }
        }
        Ok(())
    }

    fn join_video_paths(video_paths: &[&Path]) -> String {
        video_paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 执行自定义命令，将视频路径作为标准输入传入
    ///
    /// # 参数
    /// * `cmd` - 要执行的命令字符串
    /// * `video_paths` - 视频文件路径列表
    async fn execute_command(&self, cmd: &str, src: &[u8]) -> AppResult<()> {
        // 1. 跨平台 Shell 处理 (对应 shell=True)
        // Windows 使用 "cmd /C"，Unix/Mac 使用 "sh -c"
        let (shell, flag) = if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };
        // 执行自定义命令
        // 解析命令和参数
        // 启动子进程，配置标准输入管道
        let mut process = Command::new(shell)
            .arg(flag)
            .arg(cmd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .change_context(AppError::Unknown)?;

        // 将自定义输入写入标准输入
        if let Some(mut stdin) = process.stdin.take() {
            stdin
                .write_all(src)
                .await
                .change_context(AppError::Unknown)?;
        }

        let stdout = process.stdout.take().unwrap();
        let stderr = process.stderr.take().unwrap();

        let mut stdout_lines = BufReader::new(stdout).lines();
        let mut stderr_lines = BufReader::new(stderr).lines();

        loop {
            tokio::select! {
                line = stdout_lines.next_line() => {
                    match line.change_context(AppError::Unknown)? {
                        Some(l) => tracing::info!(target="user_cmd_stdout", "{}", l),
                        None => break, // stdout EOF
                    }
                }
                line = stderr_lines.next_line() => {
                    match line.change_context(AppError::Unknown)? {
                        Some(l) => tracing::warn!(target="user_cmd_stderr", "{}", l),
                        None => break, // stderr EOF
                    }
                }
            }
        }

        // 等待进程完成并检查退出状态
        let status = process.wait().await.change_context(AppError::Unknown)?;

        if !status.success() {
            bail!(AppError::Custom(format!(
                "Command failed with status: {}",
                status
            )));
        }

        Ok(())
    }

    async fn execute_webhook(&self, url: &str, src: &[u8]) -> AppResult<()> {
        let content_type = if src.is_empty() {
            "application/octet-stream"
        } else if serde_json::from_slice::<serde_json::Value>(src).is_ok() {
            "application/json"
        } else {
            "text/plain; charset=utf-8"
        };

        let response = reqwest::Client::new()
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(src.to_vec())
            .send()
            .await
            .change_context(AppError::Unknown)?;

        let status = response.status();
        let body = response.text().await.change_context(AppError::Unknown)?;

        if !status.is_success() {
            bail!(AppError::Custom(format!(
                "Webhook failed with status {status}: {}",
                body.trim()
            )));
        }

        if !body.trim().is_empty() {
            info!(url, response = body.trim(), "webhook executed");
        } else {
            info!(url, "webhook executed");
        }

        Ok(())
    }

    /// 移动文件到指定目录
    ///
    /// # 参数
    /// * `video_paths` - 视频文件路径列表
    /// * `target_dir` - 目标目录路径
    async fn move_file(&self, video_paths: &[&Path], target_dir: &str) -> AppResult<()> {
        let target_path = Path::new(target_dir);

        if !target_path.exists() {
            fs::create_dir_all(target_path)
                .await
                .change_context(AppError::Unknown)?;
        }

        for video_path in video_paths {
            self.move_single_file(video_path, target_path).await?;

            // 移动对应的 XML 文件
            let xml_path = video_path.with_extension("xml");
            if xml_path.exists() {
                // info!("移动弹幕文件: {}", xml_path.display());
                self.move_single_file(&xml_path, target_path).await?;
            }
        }
        Ok(())
    }

    /// 移动单个文件，支持跨文件系统
    async fn move_single_file(&self, source: &Path, target_dir: &Path) -> AppResult<()> {
        let file_name = source
            .file_name()
            .ok_or(AppError::Custom("Invalid file name".to_string()))?;
        let destination = target_dir.join(file_name);

        info!("Moving {} to {}", source.display(), destination.display());

        // 先尝试 rename（快速，仅同文件系统）
        match fs::rename(source, &destination).await {
            Ok(_) => Ok(()),
            Err(e) => {
                // 检查是否是跨文件系统错误
                if HookStep::is_cross_device_error(&e) {
                    info!("检测到跨文件系统操作，使用复制+删除模式");
                    // 复制文件
                    fs::copy(source, &destination)
                        .await
                        .change_context(AppError::Unknown)?;

                    // 删除原文件
                    fs::remove_file(source)
                        .await
                        .change_context(AppError::Unknown)?;

                    Ok(())
                } else {
                    Err(e).change_context(AppError::Unknown)?
                }
            }
        }
    }

    /// 判断是否是跨设备/文件系统错误
    fn is_cross_device_error(error: &std::io::Error) -> bool {
        #[cfg(unix)]
        {
            error.raw_os_error() == Some(libc::EXDEV)
        }
        #[cfg(windows)]
        {
            error.raw_os_error() == Some(17) // ERROR_NOT_SAME_DEVICE
        }
        #[cfg(not(any(unix, windows)))]
        {
            error.kind() == std::io::ErrorKind::Other
        }
    }

    /// 删除指定的视频文件
    ///
    /// # 参数
    /// * `video_paths` - 要删除的视频文件路径列表
    pub(crate) async fn remove_file(video_paths: &[&Path]) -> AppResult<()> {
        // 逐个删除视频文件
        for video_path in video_paths {
            info!("删除 - Removing: {}", video_path.display());
            fs::remove_file(video_path)
                .await
                .change_context(AppError::Unknown)?;
            let buf = video_path.with_extension("xml");
            if buf.exists() {
                fs::remove_file(&buf)
                    .await
                    .change_context(AppError::Unknown)?;
                info!("删除弹幕文件 - Removing: {}", buf.display());
            }
        }
        Ok(())
    }
}

/// 处理所有后处理器步骤
/// 处理视频文件的主函数
/// 按顺序执行所有处理器步骤
///
/// # 参数
/// * `video_path` - 视频文件路径列表
/// * `processors` - 处理器步骤列表
pub async fn process_video(
    video_path: &[&Path],
    processors: &[HookStep],
    webhook_input: Option<&[u8]>,
) -> AppResult<()> {
    // info!("Starting video processing...");

    // 依次执行每个处理器步骤
    for processor in processors {
        match processor {
            HookStep::Webhook { .. } => {
                if let Some(input) = webhook_input {
                    processor.execute_with(input).await?;
                } else {
                    processor.execute(video_path).await?;
                }
            }
            _ => processor.execute(video_path).await?,
        }
    }

    // info!("Video processing completed");
    Ok(())
}

pub async fn process(input: &[u8], processors: &Option<Vec<HookStep>>) {
    if let Some(hooks) = processors {
        // 依次执行每个处理器步骤
        for processor in hooks {
            match processor {
                HookStep::Run { run } => info!(cmd=%run),
                HookStep::Move { mv } => info!(cmd=%mv),
                HookStep::Webhook { webhook } => info!(cmd=%webhook),
                HookStep::Remove(s) => info!(cmd=%s),
            }
            if let Err(e) = processor.execute_with(input).await {
                error!(error=?e, "自定义处理执行出错");
            }
            // info!(processor=?processor, "processing completed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HookStep;

    #[test]
    fn deserialize_legacy_hook_step_shapes() {
        let run: HookStep = serde_json::from_str(r#"{"run":"echo hi"}"#).unwrap();
        let mv: HookStep = serde_json::from_str(r#"{"mv":"backup/"}"#).unwrap();
        let webhook: HookStep =
            serde_json::from_str(r#"{"webhook":"https://example.com/hook"}"#).unwrap();
        let rm: HookStep = serde_json::from_str(r#""rm""#).unwrap();

        assert_eq!(
            run,
            HookStep::Run {
                run: "echo hi".into()
            }
        );
        assert_eq!(
            mv,
            HookStep::Move {
                mv: "backup/".into()
            }
        );
        assert_eq!(
            webhook,
            HookStep::Webhook {
                webhook: "https://example.com/hook".into()
            }
        );
        assert_eq!(rm, HookStep::Remove("rm".into()));
    }

    #[test]
    fn deserialize_cmd_value_shape() {
        let run: HookStep = serde_json::from_str(r#"{"cmd":"run","value":"echo hi"}"#).unwrap();
        let webhook: HookStep =
            serde_json::from_str(r#"{"cmd":"webhook","value":"https://example.com/hook"}"#)
                .unwrap();
        let rm: HookStep = serde_json::from_str(r#"{"cmd":"rm"}"#).unwrap();

        assert_eq!(
            run,
            HookStep::Run {
                run: "echo hi".into()
            }
        );
        assert_eq!(
            webhook,
            HookStep::Webhook {
                webhook: "https://example.com/hook".into()
            }
        );
        assert_eq!(rm, HookStep::Remove("rm".into()));
    }

    #[test]
    fn serialize_to_legacy_shape_for_storage() {
        let run = serde_json::to_string(&HookStep::Run {
            run: "echo hi".into(),
        })
        .unwrap();
        let webhook = serde_json::to_string(&HookStep::Webhook {
            webhook: "https://example.com/hook".into(),
        })
        .unwrap();
        let rm = serde_json::to_string(&HookStep::Remove("rm".into())).unwrap();

        assert_eq!(run, r#"{"run":"echo hi"}"#);
        assert_eq!(webhook, r#"{"webhook":"https://example.com/hook"}"#);
        assert_eq!(rm, r#""rm""#);
    }
}
