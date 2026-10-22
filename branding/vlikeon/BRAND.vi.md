# Hướng Dẫn Biểu Tượng Trong Suốt — Vlikeon Browser

**Thương hiệu:** Vlikeon Browser — *Gaming Superbrowser by Vlike*  
**Phương án:** Concept D ("Open View") — Bộ xuất viền Neon trong suốt hoàn toàn (*Fully Transparent Outlines*).

---

## 1. Hành Vi Trong Suốt (Transparency Architecture)

Bộ biểu tượng `final-transparent/` loại bỏ hoàn toàn các khối nền tối (dark fill) và không sử dụng bất kỳ thẻ chữ nhật nền toàn khung (`<rect>`):
* **Bên ngoài khung hình:** Trong suốt 100%.
* **Bên trong lòng từng cánh khung (bracket interior):** Trong suốt 100% (`fill="none"`).
* **Khe rãnh trung tâm và chữ V không gian âm:** Trong suốt 100%.
* **Yếu tố duy nhất hiển thị:** Đường viền nét neon phát quang sắc bén định hình cấu trúc hai cánh mở.

---

## 2. Bảng Màu Sắc Cyberpunk Neon

| Màu sắc | Mã HEX | Ứng dụng |
| :--- | :--- | :--- |
| **Electric Cyan** | `#00E5FF` | Đỉnh tab trình duyệt (0% - 20%), công nghệ mở, tốc độ cao |
| **Hyper Violet** | `#885CFF` | Thân V chuyển tiếp (Master/Display), chiều sâu bí ẩn |
| **Bright Violet** | `#A366FF` | Tinh chỉnh độ sáng thân V cho bản nhỏ 16–32px |
| **Hot Magenta** | `#FF2BD6` | Đáy khe thoát xung lực (85% - 100%) |
| **Deep Cyan (Light)** | `#008CA3` | Viền bản nền sáng (tăng độ bão hòa, chống lóa) |
| **Deep Violet (Light)** | `#5B21B6` | Thân V bản nền sáng |
| **Deep Magenta (Light)**| `#C026D3` | Đáy bản nền sáng |
| **Monochrome** | `#000000` | Nét viền 1 màu cho in ấn đơn sắc |

---

## 3. Danh Mục 5 Biến Thể SVG (viewBox 0 0 256 256)

1. **`vlikeon-open-view-neon.svg` (Master Viền Neon):**  
   Đường nét gradient 6px, `fill="none"`, không filter, trong suốt hoàn toàn trừ nét vẽ. Dùng cho giao diện tối (Dark Mode), taskbar, header web.
2. **`vlikeon-open-view-display.svg` (Display Hào Quang Nhẹ):**  
   Nét viền 6px kết hợp hào quang nhẹ qua bộ lọc 2 tầng (`stdDeviation` 4 và 1.5) với bước suy giảm alpha rõ ràng qua `<feColorMatrix>`, không dùng dark fill, không làm bít khe hở đáy.
3. **`vlikeon-open-view-light.svg` (Bản Nền Sáng):**  
   Nét viền 6px chuyển sắc sẫm màu hơn, `fill="none"`, không dùng filter, hiển thị rõ ràng trên nền trắng/xám.
4. **`vlikeon-open-view-mono.svg` (Bản Đơn Sắc):**  
   Nét viền đen `#000000` 6px, `fill="none"`, không gradient, trong suốt hoàn toàn, dành cho in ấn đơn sắc hoặc khắc chìm.
5. **`vlikeon-open-view-small.svg` (Bản Tối Ưu 16–32px):**  
   Nới rộng khe rãnh đáy lên 32 đơn vị (trước nét), tăng độ dày nét lên 14, phóng to 1.25x quanh tâm (`transform="translate(-32,-32) scale(1.25)"`), mang lại độ rộng nét thực tế ~1.1px và khe thoát đáy ~1.4px ở kích thước 16px.

---

## 4. Tích Hợp Đa Nền Tảng & Giới Hạn Cần Lưu Ý

* **Yêu cầu riêng từng nền tảng:** PNG trong suốt là tài sản thiết kế, không tự động là gói icon được mọi kho ứng dụng chấp nhận. Apple hiện hướng dẫn icon phân lớp qua Icon Composer với nhiều kiểu hiển thị, trong đó có clear/tinted; Android Adaptive Icon dùng foreground, background và lớp monochrome riêng. Bộ này cung cấp nguồn trong suốt và các định dạng xuất; cần kiểm tra bằng công cụ phát hành của nền tảng trước khi phân phối. Xem `PLATFORMS.vi.md` để biết phạm vi đã chuẩn bị và nguồn hướng dẫn chính thức.
* **Khoảng trống an toàn đề xuất:** Tối thiểu 15% kích thước biểu tượng quanh các cạnh ngoài.
* **Xác thực mã nguồn (Source Review Only):** Các thông số hình học, ma trận alpha và độ hở nét được kiểm tra trực tiếp trên mã nguồn SVG. Tiến trình kiểm thử trên thiết bị vật lý và màn hình raster thực tế **NOT RUN** bởi Antigravity (Codex thực hiện kết xuất độc lập).
