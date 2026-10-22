# Vlikeon Browser

**Gaming Superbrowser by Vlike — trình duyệt hướng tới trải nghiệm chơi game hợp nhất.**

Vlikeon Browser là tên công khai mới của dự án OmniPlay: trình duyệt dựa trên
Chromium, hướng tới kết hợp Game Hub, runtime native được cô lập, quản lý save
và điều phối remote play trong cùng một không gian làm việc.

**Dự án đang phát triển mã nguồn. Chưa có bản phát hành đã được kiểm chứng.**
Chưa chạy đợt build và kiểm thử tổng hợp; không có cam kết tương thích game,
benchmark, bộ cài được chứng nhận hay kết luận kiểm toán bảo mật.

## Định hướng

- Trình duyệt và thư viện game với quy trình quản lý phiên thống nhất.
- Tách quyền và quản lý vòng đời cho các runtime native.
- Nhập, khôi phục và đồng bộ save có mã hóa.
- Điều phối remote play và giao diện desktop, mobile, web.
- Chuẩn bị nguồn có pin phiên bản và kiểm soát riêng việc phát hành engine.

Đây là mục tiêu sản phẩm và các vùng mã nguồn đã được soạn, chưa phải danh sách
tính năng được xác nhận hoạt động. Desktop/Android hướng tới Chromium; iOS dùng
WebKit. Công việc phát triển sản phẩm đang tạm dừng theo yêu cầu của chủ dự án
để chuẩn bị công bố GitHub.

## Bắt đầu tìm hiểu

- [Tình trạng công khai](docs/PUBLIC_STATUS.md)
- [Lộ trình](ROADMAP.md)
- [Kiến trúc](docs/ARCHITECTURE.md)
- [Hướng dẫn đóng góp](CONTRIBUTING.md)
- [Báo cáo bảo mật](SECURITY.md)
- [Phạm vi giấy phép](LICENSING.md)

Repository chưa có quy trình build từ clone sạch được kiểm chứng. Các recipe
trong `tools/` là mã nguồn triển khai, không phải bằng chứng build thành công.
Thư mục `out/`, checkout upstream lớn và dữ liệu cục bộ không được đưa vào Git.

Mã gốc do dự án sở hữu áp dụng Apache-2.0 trừ khi có giấy phép riêng. Mã
Chromium và các thành phần bên thứ ba giữ nguyên giấy phép tương ứng. Định danh
`omniplay` trong mã và giao thức được giữ lại để tránh đổi tương thích thiếu
kiểm chứng; việc đổi tên công khai không đồng nghĩa đã di chuyển toàn bộ mã.
