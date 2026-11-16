# 合成 MP3 测试资产

这两个文件是本地生成的短正弦波，不来自 Suno、歌曲库或真实用户。它们只证明二进制读取、完整帧边界和实际 PCM 解码，不证明真实平台生成、模型、授权或版权权益。

生成命令（FFmpeg 仅用于制作夹具，不是 Gateway 运行时或测试执行依赖）：

```powershell
ffmpeg -nostdin -v error -f lavfi -i sine=frequency=440:duration=0.2 -map_metadata -1 -ac 2 -ar 44100 -c:a libmp3lame -b:a 128k -write_xing 0 -id3v2_version 0 synthetic-tone.mp3
ffmpeg -nostdin -v error -f lavfi -i sine=frequency=660:duration=0.2 -map_metadata -1 -ac 1 -ar 22050 -c:a libmp3lame -b:a 32k -write_xing 0 -id3v2_version 0 synthetic-mono.mp3
```

`synthetic-tone.mp3` 覆盖 MPEG-1 Layer III 双声道；`synthetic-mono.mp3` 覆盖 MPEG-2 Layer III 单声道。测试在内存中构造 ID3 标签、截断与坏帧变体，不生成临时歌曲或调用外部服务。
