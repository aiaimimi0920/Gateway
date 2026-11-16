# Symphonia MP3 解码依赖

Gateway 的 `line-suno-web-reverse-api` 通过 Cargo 可选依赖使用未修改的 Symphonia `0.5.5`，仅开启 `mp3`，不启用其他格式、网络播放器或外部 FFmpeg 运行时。

- 项目：[Project Symphonia](https://github.com/pdeljanov/Symphonia/tree/v0.5.5)。
- 版权：Copyright © 2019–2022 The Project Symphonia Developers。
- 许可：Mozilla Public License 2.0；完整许可文本见同目录 `MPL-2.0.txt`。
- 固定源码：[symphonia](https://crates.io/crates/symphonia/0.5.5)、[symphonia-core](https://crates.io/crates/symphonia-core/0.5.5)、[symphonia-bundle-mp3](https://crates.io/crates/symphonia-bundle-mp3/0.5.5)、[symphonia-metadata](https://crates.io/crates/symphonia-metadata/0.5.5)。Cargo.lock 保留各包校验和。

本批没有复制或修改库源码。Gateway 自己的帧边界准入与调用代码不是第三方库源码副本；完整解码交给该固定版本依赖。后续 P50/正式交付必须验证许可文本、此说明与源码获取方式随适用产物交付，当前源码开发通过不表示该打包门禁已经完成。
