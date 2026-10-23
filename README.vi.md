# Vlikeon Browser

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="branding/vlikeon/png/display-1024.png">
  <img src="branding/vlikeon/png/light-1024.png" width="192" alt="Logo Vlikeon Browser — khung mở viền neon">
</picture>

**Gaming Superbrowser by Vlike — trình duyệt hướng tới trải nghiệm chơi game hợp nhất.**

Vlikeon Browser là tên công khai mới của dự án OmniPlay: trình duyệt dựa trên
Chromium, hướng tới kết hợp Game Hub, runtime native được cô lập, quản lý save
và điều phối remote play trong cùng một không gian làm việc.

**Dự án đang phát triển; trình duyệt đầy đủ và bản production chưa được kiểm chứng.**
Demo Game Hub Windows riêng đã có bằng chứng chạy thử giới hạn. Bản Chromium
Windows x64 dành cho phát triển đã build hoàn chỉnh và kiểm tra tính nguyên vẹn
đầu ra; kiểm chứng vận hành vẫn còn mở. Chưa có bộ cài production được xác nhận, danh sách
tương thích game, benchmark có kiểm soát hay kết luận kiểm toán bảo mật.

## Bằng chứng hiện có

- [Build Chromium Windows](docs/WINDOWS_CHROMIUM_BUILD.md): full target `chrome`,
  nguồn đóng băng và hash đầu ra x64 được kiểm tra độc lập. Đây là bản phát triển;
  chưa nghiệm thu hành vi trình duyệt hay phát hành production.
- [Demo Windows và giới hạn](docs/PLAYABLE_DEMO.md): cửa sổ Win32 dùng WebView2,
  game Game Boy gốc và core WebAssembly. Bằng chứng demo chỉ áp dụng đúng bản đó.
- [Bản tích hợp nguồn](docs/SOURCE_INTEGRATION.md), commit `35f3271`: 53 ca Rust,
  13 ca JavaScript và 19 ca Python đạt; bốn ca Windows cần fixture riêng vẫn bỏ
  qua. Hợp đồng âm thanh giả lập và build worker/helper với cấu hình fixture đã
  đạt. Sinh lại tài nguyên cho kết quả khớp 435 tệp.
- [Danh sách công việc 1–109](docs/OUTSTANDING_WORK.md) là thứ tự hiện hành;
  số nhóm 1–17 cũ dùng để tra cứu các mảng sản phẩm.

Không cộng các ca trùng với đợt kiểm thử trước thành thành tích mới. Antigravity
chỉ thực thi kiểm thử; Codex xem kết quả, quyết định và thực hiện sửa lỗi.

## Định hướng

- Trình duyệt và thư viện game với quy trình quản lý phiên thống nhất.
- Tách quyền và quản lý vòng đời cho các runtime native.
- Nhập, khôi phục và đồng bộ save có mã hóa.
- Điều phối remote play và giao diện desktop, mobile, web.
- Chuẩn bị nguồn có pin phiên bản và kiểm soát riêng việc phát hành engine.

Đây là mục tiêu sản phẩm và các vùng mã nguồn đã được soạn, chưa phải danh sách
tính năng được xác nhận hoạt động. Desktop/Android hướng tới Chromium; iOS dùng
WebKit. Mã nguồn đã công khai trên GitHub và công việc phát triển đang tiếp tục.

## Bắt đầu tìm hiểu

- [Tình trạng công khai](docs/PUBLIC_STATUS.md)
- [Lộ trình](ROADMAP.md)
- [Kiến trúc](docs/ARCHITECTURE.md)
- [Hướng dẫn đóng góp](CONTRIBUTING.md)
- [Báo cáo bảo mật](SECURITY.md)
- [Phạm vi giấy phép](LICENSING.md)
- [Logo trong suốt và bộ icon nền tảng](branding/vlikeon/README.vi.md)

Repository chưa có quy trình build từ clone sạch được kiểm chứng. Các recipe
trong `tools/` là mã nguồn triển khai, không phải bằng chứng build thành công.
Thư mục `out/`, checkout upstream lớn và dữ liệu cục bộ không được đưa vào Git.

Mã gốc do dự án sở hữu áp dụng Apache-2.0 trừ khi có giấy phép riêng. Mã
Chromium và các thành phần bên thứ ba giữ nguyên giấy phép tương ứng. Định danh
`omniplay` trong mã và giao thức được giữ lại để tránh đổi tương thích thiếu
kiểm chứng; việc đổi tên công khai không đồng nghĩa đã di chuyển toàn bộ mã.
