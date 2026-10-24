# Báo cáo batch

- Batch / số mục gốc:
- Owner / cuộc trò chuyện App / workspace:
- Nhánh / commit bắt đầu / commit kết thúc:
- Những file thuộc batch:
- Mục tiêu hành vi và luồng caller đã nối:
- Implementation đã thay đổi:
- Xử lý lỗi / cancel / revoke / cleanup / rollback:
- Kết quả: NOT_STARTED | IN_PROGRESS | SOURCE_READY_UNVERIFIED | WAITING_PERMISSION_OR_RESOURCE | VERIFIED | ACCEPTED
- Đã chạy lệnh nào (không chạy build/test khi còn hoãn):
- Bằng chứng (commit, artifact/log, môi trường/thiết bị khi có):
- Những gì chưa chạy/chưa chứng minh:
- Thiếu sót còn lại theo từng số mục:
- Quyền/tài nguyên cần thêm, lệnh dự kiến và tác động:
- Thay đổi chưa commit của owner khác đã bảo toàn:
- Ledger / tài liệu trạng thái đã cập nhật:
- Batch tiếp theo và phụ thuộc:

Chỉ chuyển VERIFIED/ACCEPTED khi chứng cứ đáp ứng đúng phạm vi mục gốc. Tool SUCCESS, source diff hoặc test giả lập không chứng minh app/thiết bị/mạng thật đã đạt.
