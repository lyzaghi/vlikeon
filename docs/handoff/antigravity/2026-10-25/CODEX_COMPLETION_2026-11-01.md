# Codex tiếp nhận phần còn lại sau 600c254

- Batch / mục gốc: bổ sung fixture của mục 47; đồng bộ ledger mục 43 và 46–49.
- Owner / workspace: Codex, theo lựa chọn của chủ dự án khi Antigravity thiếu credits; `E:\V10\OmniPlay`.
- Nhánh / commit bắt đầu: `main`, `600c25467bd1de2ae40a1b7bb5ba8398b90602f3`.
- Checkpoint source kết thúc: `4cd64d9cce3e73ba579c929597164b98bc915164`. Ledger được commit riêng sau checkpoint này để ghi SHA đã tồn tại.
- Kết quả: **SOURCE_READY_UNVERIFIED**. Đây là hoàn thiện phần source/fixture còn thiếu trong lần bàn giao này; chưa nghiệm thu runtime hay toàn bộ dự án.

## Source đã bổ sung

`platform/core/src/backup_recovery_tests.rs` có fixture `salvaged_save_heads_recover_after_every_injected_interruption` với hai generation có quan hệ parent/child trong slot 3. Fixture chọn generation cũ làm head, đóng kết nối SQLite rồi tạo một session có khóa ngoại không hợp lệ trong profile tạm. Lỗi này kích hoạt diagnosis/recovery thật mà vẫn giữ save/head đọc được; production tự tạo snapshot và journal.

Cả năm điểm gián đoạn được đưa vào fixture. Các assertion yêu cầu snapshot chứa đầy đủ hai metadata gốc và head đã chọn; mở lại qua `Database::open` phải khôi phục đúng payload, kind/slot/generation, nguồn metadata và lineage gốc, head cũ đã chọn và liên kết quarantine. Local save mới là các nhánh độc lập, có ID mới và không có parent local; lineage gốc phải nằm nguyên trong `SaveOrigin.imported_from`, không gán nhầm quan hệ giữa các ID cũ/mới. Lần mở thứ hai phải giữ nguyên ID, origin, metadata và head, không thêm generation. Database trong quarantine và snapshot phải giữ nguyên byte qua cả hai lần mở.

Fixture mất toàn bộ metadata `corrupt_database_recovery_resumes_after_every_injected_interruption` được bổ sung assertion không tự tạo save/head, giữ payload gốc tại đường dẫn cũ với đúng từng byte sau recovery và lần mở lại, giữ nguyên game ID sau lần mở thứ hai, cùng database/WAL trong quarantine.

Không thay đổi API hoặc implementation production trong batch này. Giới hạn allocation retainer và cleanup restore-token của `600c254` được giữ nguyên, vẫn cần kiểm chứng khi tiếp tục execution.

## Ledger và tài liệu

- Mục 43: bổ sung `--output <new_dir>` vào lệnh qualification audio còn hoãn.
- Mục 46–49: `end_commit` trỏ tới checkpoint source `4cd64d9cce3e73ba579c929597164b98bc915164`, bao gồm các sửa trước đó ở `2380f3f` và `600c254`. Đây là checkpoint tích lũy, không có nghĩa batch mới sửa implementation của cả bốn mục.
- Mục 47: bổ sung owner Codex, fixture mới và các lệnh recovery cần chạy sau. Các trạng thái vẫn là `SOURCE_READY_UNVERIFIED`; accepted evidence ở các dòng khác giữ nguyên.
- `docs/PROFILE_RECOVERY_AND_QUOTA.md`: ghi phạm vi fixture, ý nghĩa local branch/original lineage và các giới hạn chứng cứ.

## Đã làm và chưa chứng minh

Chỉ đọc source/migrations/API, rà diff, chạy `git diff --check`, kiểm trạng thái Git và tạo commit cục bộ có phạm vi. `git diff --check` không báo lỗi whitespace; đây không phải kết quả compiler hay test.

Không chạy compiler, build, test, assemble, benchmark, SSH; không push/release. Các fixture mới chưa được biên dịch hoặc thực thi. Fault injection hiện trả lỗi và unwind bình thường, chưa chứng minh khả năng chịu hard process crash, mất điện, lỗi thiết bị lưu trữ hay các nền tảng/thiết bị thật.

Ba thay đổi ngoài batch được bảo toàn và không đưa vào commit: `apps/ios/OmniPlay.xcodeproj/xcshareddata/xcschemes/OmniPlay.xcscheme`, `browser/patches/android-series.json`, tệp chưa track `=`.

## Hàng đợi còn lại khi cho phép execution

Chạy từ thư mục gốc repository, chưa chạy trong phiên này:

```powershell
cargo test -p omniplay-core salvaged_save_heads_recover_after_every_injected_interruption
cargo test -p omniplay-core corrupt_database_recovery_resumes_after_every_injected_interruption
cargo test -p omniplay-core interrupted_recovery_fails_closed_on_missing_or_tampered_snapshot_and_journal
cargo test -p omniplay-core staging_meta_with_inflated_counts_fails_closed_without_allocating
```

- Mục 46: compile Rust/Java/Swift và kiểm luồng export/inspect/restore/cancel trên desktop/mobile, gồm Busy/timeout, token dùng một lần và checksum mismatch.
- Mục 47: chạy các fixture recovery, rồi qualification hard crash/mất điện/lỗi storage với kiểm tra bảo toàn dữ liệu.
- Mục 48: qualification bộ đĩa nhiều tệp và format thực theo ledger.
- Mục 49: compile và qualification NTFS sparse/compressed/hard-link, volume có cluster khác 4 KiB, số đo suy giảm và filesystem POSIX.
- Mục 43: qualification audio cần binary và endpoint thật; mỗi lần chạy dùng thư mục output mới.

Không cần xin quyền hay tài nguyên bổ sung để hoàn thiện source/ledger lần này. Execution vẫn hoãn theo yêu cầu hiện tại của chủ dự án.
