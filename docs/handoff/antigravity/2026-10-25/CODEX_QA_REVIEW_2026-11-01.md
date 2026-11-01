# Codex đối chiếu QA Antigravity — 2026-11-01

Đã xử lý các lỗi chặn QA và có kết quả đạt cho các kiểm tra trong phạm vi dưới đây. **Chưa nghiệm thu toàn bộ ứng dụng hoặc các mục 42–49.** Ledger giữ `SOURCE_READY_UNVERIFIED` vì vẫn còn build nền tảng, kiểm thử tích hợp và thiết bị thật.

Codex quyết định và sửa source. Antigravity chỉ chạy kiểm tra và báo cáo qua phiên riêng, model xác nhận từ init event là `gemini-3.8-flash-high`, high effort. Codex đọc lại log, mã thoát và inventory của từng snapshot; trạng thái SUCCESS của bridge không được dùng thay kết quả test.

## Source và các sửa đổi

GitHub main trước vòng sửa này là `b7db0d9b77294814306f1121389a76da98365f9e`.

- `6a67d648d154de299a9f1fc3a83262a1ebd4e3c0`: bổ sung import Rust còn thiếu, làm rõ kiểu raw pointer Windows; xử lý sự kiện thay đổi display scale trong hai SDL frontend. Sửa harness media/input để kiểm đúng implementation, tiếp tục các suite độc lập khi có lỗi và ghi bằng chứng lỗi đầy đủ; thêm hai regression test cho chính harness.
- `fb3d85468d51b2f1bf3102bb36b191fff1d79f17`: bỏ `Default` không hợp lệ của `BlobRetentionSummary` thay vì tạo hash giả; kiểm tra pressure input ở đúng provider và yêu cầu lời gọi shaping policy.
- `f64e9d25ce666eae466f13f3c4be5933580f6765`: fixture CUE/M3U lấy dữ liệu qua public importer thật, rồi chỉ sao chép content-addressed blobs sang profile trống. Kiểm tra tái dựng CUE một đĩa và M3U hai đĩa, giữ nguyên identity, cấu trúc media, thứ tự đĩa, tổng byte và payload. Sửa tài liệu tên track chuẩn hóa thành `track-0001.bin`/`.wav`.
- `f9f4d5577e4ed6acf46b3dcf4b85c91cae2a0c27`: bổ sung tạo thư mục assets còn thiếu trong setup fixture trống. Đây là checkpoint source cuối của vòng này.

Parser production tiếp tục yêu cầu tên media chuẩn hóa. Đề xuất mở parser để chấp nhận `disc.bin` đã được Codex đối chiếu với importer và không áp dụng.

## Kết quả đã đối chiếu

| Snapshot / vòng | Kiểm tra | Kết quả thực tế |
| --- | --- | --- |
| `b7db0d9` / đầu tiên | Assemble web `--without-cores`; syntax 6 file JS | Đạt; chỉ là assemble/parser, không phải runtime |
| `6a67d64` / round 2 | Regression của harness | 2/2 đạt: lỗi assertion và lỗi đọc source vẫn giữ kết quả ba suite còn lại, mã thoát khác 0 |
| `fb3d854` / round 3 | Module `backup_recovery_tests` | Biên dịch thành công; 14 đạt, 1 lỗi ở fixture CUE dùng tên chưa chuẩn hóa |
| `fb3d854` / round 3 | Broker, local-service, mobile Rust callers | `cargo check` đạt, exit 0 |
| `fb3d854` / round 3 | Video/audio/gamepad/touch contract | 4/4 suite, 18/18 check đạt; mức `source_contract_and_synthetic_protocol`; mọi cờ live execution đều false |
| `f64e9d2` / round 4 | Fixture CUE/M3U mới | Biên dịch thành công, test lỗi vì setup thiếu thư mục assets; bằng chứng được giữ nguyên |
| `f9f4d55` / round 5 | Chạy lại đúng fixture CUE/M3U | 1/1 đạt, exit 0; thực thi cả CUE và M3U hai đĩa |

Có kết quả đạt cho **15 test recovery khác nhau qua round 3 và round 5**. Không chạy lại toàn module trên checkpoint cuối: sau round 3 chỉ sửa fixture CUE/M3U và tài liệu, các implementation và test khác giữ nguyên. Không mô tả kết quả này thành một lượt `15/15` trên cùng SHA.

Các ca đã đạt gồm profile backup single/multi-save và trailer bounds; staging-count không cấp phát từ số đếm sai; retainer chunk và pin nhiều archive; inspect/confirm/cancel/archive swap; snapshot/journal thiếu hoặc hỏng; orphan payload; quota/hard-link/cluster rounding. Cả hai fixture ngắt recovery đều đạt: trường hợp metadata mất hoàn toàn giữ orphan và quarantine, và trường hợp snapshot hợp lệ khôi phục hai generation cùng head được chọn trước đó.

## Tái hiện và bằng chứng

Windows MSVC x64, Rust/Cargo 1.98.0, Python 3.12, cache dependency có sẵn, hai build job; runner dùng snapshot tạo bởi `git archive`, không chứa ba thay đổi riêng của owner. Các lệnh chạy từ root của snapshot, sau khi khởi tạo MSVC environment:

```powershell
cargo test --locked --offline -p omniplay-core --lib backup_recovery_tests -- --test-threads=1
cargo check --locked --offline -p omniplay-broker -p omniplay-local-service -p omniplay-mobile
python qa/native-lifecycle/media_input_qualification.py --suite all --output <new_dir>
python qa/native-lifecycle/test_media_input_qualification.py
cargo test --locked --offline -p omniplay-core --lib library_reconstruct_rebuilds_normalized_cue_and_m3u_disc_sets -- --test-threads=1
```

[Gói bằng chứng](qa-2026-11-01/manifest.json) lưu SHA source của từng vòng, raw log, kết quả runner, integrity và SHA-256 của các artifact. Round 3, 4, 5 đều ghi 1.875 file gốc không thay đổi sau chạy. Vòng đầu có một receipt assemble sinh tự động thay đổi trong snapshot QA; workspace Git chính không bị Antigravity sửa. Inventory chỉ xác nhận các file gốc được hash, không phải filesystem sandbox.

Nhật ký phiên bridge và các snapshot đầy đủ được giữ tại thư mục work của máy thực hiện. Gói trong Git chứa bằng chứng đã chọn và lịch sử lỗi, không chứa credential, cache build hoặc transcript toàn bộ hội thoại.

## Các việc vẫn còn thiếu

1. Biên dịch và chạy native C++ frontend với thay đổi SDL display scale; Rust checks và source contracts không biên dịch C++.
2. Build Java/Swift và thử Android/iOS thật. Rust `omniplay-mobile` check không chứng minh native adapters build hoặc hoạt động.
3. Kiểm thử tích hợp desktop/mobile profile export/inspect/restore/cancel: owner isolation, Busy/timeout, native picker và checksum mismatch. Unit test capability broker đã đạt nhưng chưa bao phủ toàn bộ IPC/UI.
4. Hard process crash, mất điện và lỗi storage thật. Fault injection hiện tại trả lỗi và unwind bình thường.
5. Bộ đĩa CHD/ISO/PBP, binding mơ hồ và các trường hợp nhiều bộ đĩa khác. Lượt tập trung mới chỉ chứng minh CUE và M3U hai CUE do importer tạo.
6. Đo quota trên sparse/compressed files, volume Windows có cluster khác, Linux/macOS và filesystem lỗi. Ca cluster rounding hiện tại là unit test.
7. Live GPU/DPI/context/device loss, audio interruption/consumer stall, USB/Bluetooth gamepad và touch/rotation trên thiết bị thật.

Các warning compiler hiện có được giữ trong log, chưa được coi là lỗi build. Không chạy Linux/SSH, build Chromium hoặc qualification phần cứng trong vòng này. Những dòng “chưa chạy” trong hồ sơ authoring trước đây là trạng thái lịch sử; bảng trên là cập nhật thực thi mới nhất cho đúng phạm vi nêu tại đây.
