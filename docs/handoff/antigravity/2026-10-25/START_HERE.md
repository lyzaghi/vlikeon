# Prompt giao việc cho Antigravity App

Mở workspace dự án Vlikeon: `E:\V10\OmniPlay`. Đây là lời giao việc của chủ dự án: tiếp tục hoàn thiện toàn bộ công việc còn thiếu, gồm triển khai mã nguồn và sửa lỗi, không còn giới hạn chỉ test trong phân công cũ.

Đọc `docs/ANTIGRAVITY_HANDOFF_2026-10-25.md`, `docs/OUTSTANDING_WORK.md`, `docs/handoff/antigravity/2026-10-25/WORK_ITEMS.csv` và hướng dẫn AGENTS áp dụng trước khi sửa file. Checkpoint source của chat bàn giao là `c795772`; tài liệu bàn giao được thêm sau checkpoint này. Dùng HEAD hiện tại và tình trạng workspace làm căn cứ, không reset về checkpoint.

Mục tiêu đầy đủ là 1–109 trong sổ toàn dự án. Điểm tiếp tục của chat source này là 51–109; 51–52 có bằng chứng đã chấp nhận, tiếp theo là các thiếu sót source 53–60 rồi 61–109. Các việc 4–49 còn mở phải giữ trong ledger và xác định owner với công việc desktop/core ở chat khác; không bỏ sót hoặc sửa đè.

Ưu tiên viết và nối implementation/caller/packaging thực tế. Build/test/benchmark/SSH/deploy vẫn hoãn trong phiên này. Chuẩn bị lệnh và acceptance queue để chạy sau, không chạy ngay và không báo PASS. Không bật streaming policy, thay trust root, tạo khóa phát hành hay sửa quyền hệ thống.

Có thay đổi chưa commit ở iOS scheme và Android patch series khi bàn giao; bảo toàn chúng. Ưu tiên workspace/nhánh riêng, không sửa đồng thời file cùng owner khác, không stage/commit ké. Không force push hoặc phát hành sản phẩm.

Bắt đầu bằng việc ghi ngắn gọn HEAD/workspace, batch gốc sẽ nhận và file định sửa. Chỉ đọc source cần cho batch. Tiếp tục hoàn thiện capture Linux/macOS, gamepad và native video service còn thiếu, rồi nối transport ICE/DTLS/SRTP thực tế với luồng broker nhận/giải mã/trình bày; các adapter/parser hiện có chưa phải sản phẩm chạy đầu cuối. Không dành cả lượt để chỉ viết kế hoạch mới hoặc thêm wrapper không có caller.

Giữ original packet receipt, grant/owner/document/generation, bounded queues và cleanup đã xác nhận. Không plaintext fallback, không fake success, không xóa nợ cleanup theo TTL. Dữ liệu bí mật không đưa vào log/prompt/repo.

Cuối mỗi batch dùng `BATCH_REPORT_TEMPLATE.md`, cập nhật từng hàng ledger và commit chỉ file của batch. Phân biệt source đã viết, source chưa kiểm chứng, thiếu quyền/tài nguyên và hoàn tất có bằng chứng. Chưa được đánh dấu xong toàn dự án cho đến khi nghiệm thu từng mục và chốt 109.
