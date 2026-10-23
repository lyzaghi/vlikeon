# Sequential outstanding work — owner-approved numbering

The owner requests completing these items in order, without changing scope.
Antigravity executes assigned tests only; Codex owns review, fixes and acceptance.
These are the new 1–109 identifiers, distinct from the historical 17-area roadmap.
Unchecked items remain open, including items with authored but unqualified source.

Owner steering on 2026-10-23: defer Linux build/runtime work in item 53 and
continue with item 54 (macOS capture). Item 53 remains open; no Linux host,
VM installation, SSH access or new Linux build/test is authorized by this move.
The owner subsequently deferred macOS build/test and requested the next step:
item 55 source work. All new build/test execution stays deferred, including
Antigravity batches, while source work continues. Items 53 and 54 remain open.

1. [x] Tích hợp các thay đổi mã nguồn đang nằm ngoài commit thành các phiên bản thống nhất, có thể tái tạo. Bằng chứng và giới hạn: `SOURCE_INTEGRATION.md`.
2. [x] Đồng bộ README, roadmap và tài liệu trạng thái với kết quả phát triển, kiểm thử thực tế. Đối chiếu: `DOCUMENTATION_ALIGNMENT.md`.
3. [ ] Build trình duyệt Chromium Vlikeon hoàn chỉnh trên Windows.
4. [ ] Build trình duyệt Chromium Vlikeon hoàn chỉnh trên Linux.
5. [ ] Build trình duyệt Chromium Vlikeon hoàn chỉnh trên macOS.
6. [ ] Kiểm chứng Game Hub, tab, hồ sơ thường/riêng tư, quyền truy cập và native bridge trong trình duyệt thật.
7. [ ] Kiểm chứng quy trình nâng phiên bản Chromium mà vẫn giữ đúng các bản vá Vlikeon.
8. [ ] Hoàn thiện giới hạn dung lượng ghi của core, helper, bộ nhớ đệm và hồ sơ trên Windows.
9. [ ] Hoàn thiện phục hồi và xử lý dữ liệu phiên Windows cũ thiếu bằng chứng sở hữu.
10. [ ] Hoàn thiện bộ dependency và triển khai tách biệt worker chạy core với cửa sổ trình bày trên Windows.
11. [ ] Kiểm chứng toàn bộ vòng đời AppContainer, Job Object, IPC và thu hồi tiến trình trên Windows.
12. [ ] Hoàn thiện gói broker, worker và SDL presentation dùng thực tế trên Linux.
13. [ ] Hoàn thiện sửa chữa và di chuyển cấu hình cây tài nguyên được systemd cấp quyền trên Linux.
14. [ ] Hoàn thiện phục hồi khi tạo hoặc đóng phiên Linux bị gián đoạn.
15. [ ] Hoàn thiện kích hoạt cửa sổ, focus, pause và lưu game trên X11/Wayland.
16. [ ] Hoàn thiện tính toán dung lượng vật lý của profile, image, cache và phiên Linux.
17. [ ] Kiểm chứng sandbox, systemd, cgroup và thu hồi toàn bộ cây tiến trình trên Linux thật.
18. [ ] Build kernel, chương trình khởi động và image Linux ARM64 cho môi trường giả lập macOS.
19. [ ] Hoàn thiện catalog và gói macOS có thông tin xác thực khớp chính xác các artifact thực tế.
20. [ ] Kiểm chứng khởi động VM, chạy game, trao đổi hình/âm thanh và lưu/khôi phục trên Mac thật.
21. [ ] Kiểm chứng phục hồi macOS sau crash, mất owner, khởi động lại máy và lạm dụng tài nguyên.
22. [ ] Hoàn thiện và phê duyệt core Game Boy cho sản phẩm chính thức, vượt ngoài phạm vi demo hiện tại.
23. [ ] Hoàn thiện và kiểm chứng core Game Boy Advance.
24. [ ] Hoàn thiện và kiểm chứng core NES.
25. [ ] Hoàn thiện và kiểm chứng core SNES.
26. [ ] Hoàn thiện và kiểm chứng core Nintendo DS.
27. [ ] Hoàn thiện và kiểm chứng core Mega Drive.
28. [ ] Hoàn thiện và kiểm chứng core Nintendo 64.
29. [ ] Hoàn thiện và kiểm chứng core PlayStation 1.
30. [ ] Hoàn thiện và kiểm chứng core PSP.
31. [ ] Chốt các điều kiện an toàn còn thiếu của snapshot và tài nguyên thiết bị trong core N64.
32. [ ] Hoàn thiện các đường ghi RAM, RTC, thẻ nhớ, đĩa và thu hồi writer còn mở trong core N64.
33. [ ] Chốt giới hạn bộ nhớ, tài nguyên GPU và xử lý lỗi gửi lệnh đồ họa của core N64.
34. [ ] Hoàn thiện và kiểm chứng AI FIFO, lịch ngắt, resampler và vòng đời âm thanh N64.
35. [ ] Build lại atlas/font PSP từ nguồn mở đã chọn và kiểm tra đầu ra thực tế.
36. [ ] Build lại các gói core/frontend theo đúng phiên bản giao thức input và hiển thị mới.
37. [ ] Hoàn thiện giao diện hiệu chỉnh tay cầm raw joystick cho SDL native.
38. [ ] Hoàn thiện touch overlay native ngoài lớp điều khiển Web hiện có.
39. [ ] Hoàn thiện tích hợp input native dành riêng cho Android và iOS.
40. [ ] Hoàn thiện cấu hình nút có lực nhấn và phụ kiện/controller đặc thù từng core.
41. [ ] Hoàn thiện các backend GPU bổ sung nằm trong phạm vi sản phẩm đã duyệt.
42. [ ] Kiểm chứng video khi resize, fullscreen, đổi DPI, mất context và lỗi driver.
43. [ ] Kiểm chứng âm thanh khi đổi thiết bị, gián đoạn, thiếu dữ liệu và consumer bị treo.
44. [ ] Kiểm chứng tay cầm thật, hotplug, remap, rung và giải phóng input khi mất focus.
45. [ ] Kiểm chứng trình chỉnh touch trên thiết bị thật khi xoay màn hình và chạm nhiều điểm.
46. [ ] Hoàn thiện sao lưu toàn bộ profile, vượt giới hạn sao lưu nhóm save hiện tại.
47. [ ] Hoàn thiện chẩn đoán và phục hồi cơ sở dữ liệu bị hỏng.
48. [ ] Hoàn thiện tái dựng metadata nhập game và cấu trúc bộ đĩa nhiều tệp.
49. [ ] Hoàn thiện quota vật lý và tính toán các tham chiếu đang giữ dữ liệu.
50. [x] Hoàn thiện công cụ phục hồi thủ công cho dữ liệu thiếu metadata đáng tin cậy. Bằng chứng và giới hạn: `MANUAL_DATA_RECOVERY.md`.
51. [x] Kiểm chứng trao đổi archive sao lưu giữa bản native và Web/PWA. Bằng chứng và giới hạn: `SAVE_ARCHIVE_INTERCHANGE.md`.
52. [x] Kiểm chứng lưu trữ khi đầy đĩa, crash, giao dịch bị hủy và nâng schema dữ liệu. Bằng chứng và giới hạn: `STORAGE_FAULT_QUALIFICATION.md`.
53. [ ] Hoàn thiện capture cửa sổ game và âm thanh ứng dụng trên Linux. Hoãn build/runtime theo yêu cầu chủ dự án; thiếu sót giữ tại `LINUX_OWNED_CAPTURE.md`.
54. [ ] Hoàn thiện capture cửa sổ game và âm thanh ứng dụng trên macOS. Mã đang triển khai và phần chưa kiểm chứng: `MACOS_OWNED_CAPTURE.md`.
55. [ ] Hoàn thiện tích hợp virtual gamepad và phân quyền thiết bị điều khiển từ xa. Mã quyền/slot và backend uinput đang triển khai; phần còn thiếu tại `VIRTUAL_GAMEPAD_OWNERSHIP.md`. Build/test hoãn theo yêu cầu.
56. [ ] Hoàn thiện dịch vụ capture/encode native độc lập và lựa chọn codec.
57. [ ] Hoàn thiện nhận diện quyền sở hữu tiến trình game con được launcher khởi chạy.
58. [ ] Hoàn thiện vòng đời credential ghép đôi trên native và mobile.
59. [ ] Triển khai hạ tầng signaling, relay và TURN dùng thực tế.
60. [ ] Kiểm chứng Remote Play với mạng thật, reconnect, thu hồi quyền và lỗi capture/input.
61. [ ] Hoàn thiện tích hợp core/runtime được duyệt vào sản phẩm Android.
62. [ ] Hoàn thiện Remote Play và input/capture mobile trên Android.
63. [ ] Hoàn thiện các phần đồng bộ nền và document provider Android còn thiếu.
64. [ ] Hoàn thiện nhận diện Vlikeon, bản địa hóa và accessibility trên Android.
65. [ ] Build APK/AAB Android thực tế từ Chromium, JNI và Rust.
66. [ ] Kiểm chứng Android trên thiết bị thật, gồm bộ nhớ trang 16 KiB, lifecycle, âm thanh và tay cầm.
67. [ ] Hoàn thiện core và adapter phù hợp chính sách không JIT của iOS/iPadOS.
68. [ ] Hoàn thiện Remote Play trên iOS/iPadOS.
69. [ ] Hoàn thiện các phần đồng bộ nền và File Provider iOS còn thiếu.
70. [ ] Hoàn thiện giao diện, icon sản phẩm, bản địa hóa và accessibility trên iPhone/iPad.
71. [ ] Build ứng dụng iOS bằng Xcode cho simulator và thiết bị.
72. [ ] Kiểm chứng iPhone/iPad thật về lưu nền, khôi phục, tải tệp, quyền riêng tư và điều khiển.
73. [ ] Hoàn thiện các dependency ngoại tuyến Web/PWA nằm ngoài thư mục core.
74. [ ] Hoàn thiện trải nghiệm cài đặt, cập nhật và phục hồi Web/PWA.
75. [ ] Kiểm chứng quota, bộ nhớ và hiệu năng xác minh cache trên Web/PWA.
76. [ ] Kiểm chứng Web/PWA trên các trình duyệt và thiết bị mục tiêu.
77. [ ] Hoàn thiện vòng đời tài khoản, khôi phục tài khoản và quản lý credential xuyên nền tảng.
78. [ ] Phê duyệt artifact mã hóa và kiểm chứng tương thích giữa các triển khai.
79. [ ] Triển khai dịch vụ đồng bộ có xác thực và HTTPS production.
80. [ ] Hoàn thiện thiết kế mã hóa dữ liệu lớn nhằm kiểm soát bộ nhớ và xử lý theo phân đoạn.
81. [ ] Hoàn thiện hủy truyền bất đồng bộ và đối soát thao tác khi mất phản hồi.
82. [ ] Hoàn thiện quota, dữ liệu tạm, dữ liệu mồ côi và nguồn gốc save trong đồng bộ.
83. [ ] Kiểm chứng đồng bộ đầu cuối khi revoke, xung đột, mất mạng, khóa máy và khởi động lại.
84. [ ] Hoàn thiện kho benchmark gắn với đúng runtime, cấu hình core, game và thiết bị.
85. [ ] Hoàn thiện lựa chọn runtime native/remote dựa trên registry và quyền sử dụng thực tế.
86. [ ] Bổ sung đo lường cho các đường presentation hiện chưa hỗ trợ giao thức benchmark.
87. [ ] Thực hiện benchmark có kiểm soát về FPS, độ trễ, RAM, điện năng và nhiệt độ.
88. [ ] Hoàn thiện giao diện cài đặt sản phẩm trên Windows, Linux và macOS.
89. [ ] Hoàn thiện sửa chữa chính xác một bản cài đặt đã hỏng bằng gói được ký hợp lệ.
90. [ ] Hoàn thiện gỡ cài đặt với lựa chọn giữ/xóa dữ liệu và xử lý phiên đang hoạt động.
91. [ ] Hoàn thiện bảo trì bootstrap và dọn đăng ký hệ thống trên Windows.
92. [ ] Hoàn thiện tích hợp bộ quản lý gói trên Linux.
93. [ ] Hoàn thiện đóng gói cập nhật ứng dụng macOS đã ký và notarize.
94. [ ] Hoàn thiện dọn phiên bản cũ, staging, quota và dữ liệu cập nhật bị bỏ dở.
95. [ ] Kiểm chứng cài đặt, nâng cấp, hủy, crash, repair, uninstall và rollback thực tế.
96. [ ] Hoàn tất kiểm toán giấy phép và nguồn gốc của dependency, core, codec, font và firmware được phân phối.
97. [ ] Chuẩn bị đầy đủ mã nguồn tương ứng với chính xác các binary phát hành.
98. [ ] Tạo SBOM từ artifact thực tế và đối chiếu đầy đủ thành phần được đóng gói.
99. [ ] Rà soát advisory/CVE và độ mới bảo mật của Chromium cùng các dependency.
100. [ ] Thiết lập danh tính phát hành, khóa ký, trust root và máy chủ cập nhật HTTPS production.
101. [ ] Hoàn thiện ký mã, notarization và cấu hình phát hành cho từng nền tảng.
102. [ ] Hoàn tất hồ sơ quyền riêng tư, khai báo API, xuất khẩu mã hóa và yêu cầu cửa hàng ứng dụng.
103. [ ] Thiết lập kênh tiếp nhận lỗ hổng riêng tư và chính sách hỗ trợ bảo mật.
104. [ ] Thực hiện đánh giá bảo mật độc lập đối với sandbox, quyền truy cập, cập nhật và đồng bộ.
105. [ ] Chạy bốn ca Windows đang bị bỏ qua vì cần fixture/worker riêng.
106. [ ] Chạy ma trận kiểm thử ứng dụng hoàn chỉnh trên các nền tảng và thiết bị mục tiêu.
107. [ ] Kiểm chứng build từ bản clone sạch và thiết lập kiểm thử hồi quy lặp lại trên CI.
108. [ ] Thu thập phản hồi dùng thử thật và xử lý các lỗi đã được xác nhận.
109. [ ] Thực hiện đánh giá tổng thể mục 17, chốt từng thiếu sót và quyết định mức sẵn sàng phát hành.
