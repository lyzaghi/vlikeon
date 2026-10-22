# Vlikeon Browser — Open View Neon / nền trong suốt

Nguồn vector: `svg/vlikeon-open-view-neon.svg`.
PNG lớn: `png/neon-4096.png` hoặc `png/display-4096.png` (halo nhẹ).

Tất cả PNG logo giữ alpha; không có nền đen/trắng ghép sẵn, kể cả lòng khung.
Các ảnh preview có nền kiểm tra được đặt riêng; không dùng chúng làm logo gốc.

- `windows/`: ICO 15 kích thước và PNG.
- `macos/`: ICNS và iconset, có cỡ Retina.
- `linux/`: hicolor PNG và SVG.
- `web/`: favicon, icon PWA và nguồn tích hợp tham khảo.
- `android/`: foreground trong suốt, PNG theo mật độ và nguồn Adaptive Icon.
- `apple/`: foreground PNG 1024 và SVG cho quy trình Icon Composer.

Đọc `PLATFORMS.vi.md` trước khi tích hợp. Đây là bộ tài sản thiết kế và nguồn
resource, chưa phải bằng chứng ứng dụng chạy hoặc store chấp thuận.

Kết quả kiểm tra: `PNG_VERIFICATION.json`, `ASSET_VERIFICATION.json` và
`ICON_CONTAINER_VERIFICATION.json`. Không chạy build/test sản phẩm.

Mã/đồ họa gốc của dự án áp dụng Apache-2.0 theo lựa chọn của chủ dự án; xem
`LICENSE.txt`. Giấy phép không cấp quyền bảo chứng hoặc quyền nhãn hiệu riêng.
