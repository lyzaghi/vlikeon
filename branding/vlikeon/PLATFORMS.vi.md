# Bộ icon Vlikeon Browser — Open View Neon

Thiết kế giữ phương án D đã chọn: hai nửa khung mở, viền cyan–tím–magenta,
không có nền hoặc phần tô đen bên trong. SVG là bản gốc vector. PNG xuất trực
tiếp từ SVG, có alpha thật, sRGB và nén lossless; không phóng lớn từ ảnh nhỏ.

## Tệp nên dùng

| Mục đích | Đường dẫn trong bộ |
| --- | --- |
| Logo vector trong suốt | `svg/vlikeon-open-view-neon.svg` |
| Neon có quầng sáng nhẹ | `svg/vlikeon-open-view-display.svg` |
| Nền sáng | `svg/vlikeon-open-view-light.svg` |
| Một màu đen / trắng | `svg/vlikeon-open-view-mono.svg`, `svg/vlikeon-open-view-white.svg` |
| Favicon nhỏ / nền sáng | `svg/vlikeon-open-view-small.svg`, `svg/vlikeon-open-view-small-light.svg` |
| PNG lớn trong suốt | `png/neon-4096.png`, `png/display-4096.png`, `png/light-4096.png` |
| Windows | `windows/vlikeon.ico` và PNG đa kích thước |
| macOS truyền thống | `macos/Vlikeon.icns`, `macos/Vlikeon.iconset/` |
| Linux | `linux/hicolor/`, SVG và PNG theo cỡ |
| Web / PWA | `web/`, favicon, PNG, manifest và đoạn HTML tham khảo |
| Android | `android/res/`, foreground và monochrome dạng vector, PNG theo mật độ |
| iOS / iPadOS / Apple Icon Composer | `apple/`, lớp foreground 1024 và SVG |

## Chọn biến thể theo nền

- Nền tối: dùng neon hoặc display. Display chỉ dùng khi có đủ diện tích cho halo.
- Nền sáng: dùng light để nét cyan không nhạt trên nền trắng.
- Nền phức tạp: dùng bản một màu phù hợp tương phản hoặc chọn vùng bố trí yên tĩnh.
- 16–32 px: dùng bản small; Windows bổ sung bản này đến 40 px. Các cỡ lớn hơn
  dùng master để giữ đúng tỷ lệ gốc.

Không kéo méo, nối kín khe giữa, thêm nền vào master hay dùng halo cho favicon.
Không coi một bản neon duy nhất là đủ tương phản trên mọi màu nền.

## Tính trong suốt và các lớp nền hệ thống

Các SVG/PNG logo, Windows ICO, macOS iconset/ICNS, Linux và foreground mobile
trong bộ này giữ alpha. Bảng trong `previews/` có nền kiểm tra, không phải logo
nền trong suốt. Hai bảng preview ở thư mục gốc cũng chỉ để xem thiết kế.

Android Adaptive Icon bắt buộc khai báo lớp foreground/background theo cấu trúc
nền tảng. Nguồn tham khảo trong bộ tách foreground trong suốt khỏi màu nền
`vlikeon_icon_background`; màu này chỉ có tác dụng khi hệ thống ghép launcher.
Không có nền đặc được ghép sẵn vào PNG logo. Monochrome dành cho icon theo theme,
vì vậy hệ thống có thể đổi màu neon.

Apple hiện dùng lớp foreground/background và nhiều appearance. Bộ này cung cấp
các lớp nguồn trong suốt để đưa vào Icon Composer, không giả lập hoặc tuyên bố
đã tạo tệp `.icon` chính thức. Chưa chạy Xcode/Icon Composer, trình biên dịch
resource Android, ứng dụng native hay quy trình duyệt kho.

Web manifest để `purpose: any`. Không gắn nhãn maskable cho PNG trong suốt chưa
được thiết kế và kiểm chứng theo safe zone riêng. Các màu metadata của manifest
là màu giao diện gợi ý, không phải pixel nền trong icon.

## Kiểm tra đã làm

- Đọc nguồn SVG; master không nhúng ảnh, font ngoài hoặc script; filter chỉ ở display.
- Render PNG, xem trực tiếp trên nền sáng, tối, màu và checkerboard.
- Kiểm tra pixel alpha ở góc, lòng khung và tâm; ghi trong `ASSET_VERIFICATION.json`.
- Đóng gói và đọc lại các frame ICO/ICNS bằng Pillow; xem `ICON_CONTAINER_VERIFICATION.json`.

Đây là kiểm tra tài sản thiết kế và định dạng tệp. Chưa xác nhận hiển thị trên
thiết bị thật, mọi renderer, mọi phiên bản hệ điều hành hoặc chấp thuận của store.
Các quy chuẩn và biến thể mới vẫn cần kiểm chứng khi tích hợp sản phẩm.

## Nguồn chính thức đã tham khảo ngày 2026-10-22

- [Windows icon construction](https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-construction)
- [Android Adaptive Icons](https://developer.android.com/develop/ui/compose/system/icon_design_adaptive)
- [Google Play icon specifications](https://developer.android.com/distribute/google-play/resources/icon-design-specifications)
- [Apple app icons](https://developer.apple.com/design/human-interface-guidelines/app-icons)
- [Apple Icon Composer](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer)
- [Linux icon theme specification](https://specifications.freedesktop.org/icon-theme/latest/)
- [Pillow ICO/ICNS](https://pillow.readthedocs.io/en/stable/handbook/image-file-formats.html)

Thiết kế vector do Antigravity Gemini 3.8 Flash High tạo theo lựa chọn của chủ dự
án; Codex kiểm tra, xuất định dạng, bổ sung biến thể trắng/light-small và chỉnh
lại hướng dẫn tích hợp theo phạm vi đã thực hiện. Không tuyên bố độc quyền nhãn hiệu.
