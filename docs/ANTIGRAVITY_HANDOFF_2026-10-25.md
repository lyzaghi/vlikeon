# Kế hoạch bàn giao Vlikeon cho Antigravity App

Ngày: 2026-10-25, Asia/Saigon. Đây là bộ bàn giao để tiếp tục dự án, không phải báo cáo sản phẩm đã hoàn thành.

## 1. Phạm vi và quyền thực hiện

Yêu cầu mới của chủ dự án: lập kế hoạch giao toàn bộ công việc còn lại cho Antigravity App thực hiện tiếp. Khi bắt đầu triển khai theo bộ bàn giao, Antigravity được sửa và tích hợp mã nguồn, sửa lỗi và cập nhật tài liệu liên quan; giới hạn lịch sử “Antigravity chỉ test” không còn là phân công áp dụng cho việc bàn giao này. Chủ dự án vẫn quyết định phát hành và những thay đổi phạm vi sản phẩm.

Yêu cầu này là chuẩn bị bàn giao. Việc tạo tài liệu không có nghĩa App đã nhận prompt hoặc đã chạy công việc. Không tự nhận một phiên CLI là cuộc trò chuyện trong App.

Các chỉ đạo đang giữ nguyên:

- Ưu tiên hoàn thiện source; build/test để sau. Chuẩn bị lệnh, fixture và CI được, nhưng chưa chạy build, test, benchmark, batch Antigravity hay qualification thiết bị trong phạm vi bàn giao này.
- Không bật `security/streaming-channel.json`, không thêm relay origin, không deploy production hoặc tự tạo khóa ký phát hành.
- Quyền SSH/build của cuộc trò chuyện khác không tự chuyển sang phiên Antigravity mới. Ghi nhận kết quả được cung cấp; không khởi động lại job của bên khác.
- Không cài driver, SDK hay phần mềm hệ thống mới; không thay chính sách bảo mật hoặc quyền thực thi chỉ để vượt lỗi quyền.
- Không dùng tài khoản của người khác để tạo phản hồi được trình bày như trải nghiệm độc lập. Phản hồi dùng thử phải do người dùng thật cung cấp và ghi đúng nguồn.
- Không tự thay kiến trúc, bỏ nền tảng, giảm codec/core đã duyệt, thay thư viện bằng mock hoặc đánh dấu xong bằng cách vô hiệu hóa chức năng.

Khi đến bước phụ thuộc quyền mới, ghi chính xác hạng mục, lệnh dự kiến, tài nguyên cần dùng và tiếp tục phần source độc lập. Không xin lại quyền chung đã được chủ dự án cấp.

## 2. Nguồn bàn giao và trạng thái Git

- Repository: `E:\V10\OmniPlay`.
- Remote: `https://github.com/lyzaghi/vlikeon.git`; nhánh nguồn: `main`.
- Checkpoint mã nguồn do chat này bàn giao: `c795772` (đã push trước khi lập bộ tài liệu).
- Danh sách gốc: `docs/OUTSTANDING_WORK.md`, giữ nguyên số 1–109.
- Tiến trình source: `docs/COMPLETION_SEQUENCE.md`. Đây là lịch sử nối tiếp; các đoạn cũ có thể mô tả thiếu sót đã được code mới thay thế. Đọc source hiện tại trước khi quyết định làm lại.
- Bảng bàn giao từng việc: `docs/handoff/antigravity/2026-10-25/WORK_ITEMS.csv`.
- Prompt mở đầu App: `docs/handoff/antigravity/2026-10-25/START_HERE.md`.
- Mẫu báo cáo: `docs/handoff/antigravity/2026-10-25/BATCH_REPORT_TEMPLATE.md`.

Tại thời điểm đọc repo, hai đường dẫn có thay đổi chưa commit, thuộc công việc đang có từ trước:

1. `apps/ios/OmniPlay.xcodeproj/xcshareddata/xcschemes/OmniPlay.xcscheme`.
2. `browser/patches/android-series.json`.

Không reset, clean, checkout đè, stash toàn repo hoặc commit ké hai thay đổi này. Danh sách trên là ảnh chụp lúc bàn giao, không phải bằng chứng rằng các phiên khác đã dừng. Trước khi nhận một batch, ghi lại HEAD/status của đúng workspace và xác định người đang sửa các file liên quan.

Ưu tiên checkout/worktree riêng cho phiên Antigravity, bắt đầu từ commit bàn giao thực tế bao gồm tài liệu này. Không chép cả `E:\V10`, profile, kho khóa, dữ liệu người dùng hay output build vào prompt/workspace. Nếu dùng checkout chung, phải có phân công file rõ ràng và không sửa đồng thời với chat khác. Chưa cần tạo checkout trong bước lập kế hoạch.

## 3. Sổ công việc và cách ghi trạng thái

Sáu mục được danh sách gốc đánh dấu đã chấp nhận: 1, 2, 3, 50, 51, 52. Đây là trạng thái kế thừa trong tài liệu, không phải kiểm chứng mới của Antigravity. Các mục còn lại vẫn mở; không suy từ số lượng commit thành phần trăm hoàn thành.

Phạm vi tiếp tục của chat này là 51–109: giữ bằng chứng 51–52, tiếp tục 53–109. Các việc chưa xong 4–49 vẫn nằm trong sổ toàn dự án, phải có owner rõ ràng khi chuyển giao; không được bỏ khỏi báo cáo chỉ vì nằm ngoài phạm vi chat source này.

Mỗi hàng CSV có số gốc, mô tả nguyên văn, trạng thái kế thừa, nhóm triển khai, tài liệu dẫn chiếu và trạng thái nhận việc. Khi bắt đầu, bổ sung owner/commit/chứng cứ/thiếu sót vào chính hàng đó. Không đánh lại số.

Trạng thái làm việc dùng: `NOT_STARTED`, `IN_PROGRESS`, `SOURCE_READY_UNVERIFIED`, `WAITING_PERMISSION_OR_RESOURCE`, `VERIFIED`, `ACCEPTED`. `SOURCE_READY_UNVERIFIED` không tương đương `[x]` trong danh sách gốc. Mỗi trạng thái chờ phải ghi phụ thuộc cụ thể, không ghi chung “blocked”.

## 4. Nhóm bàn giao bao phủ toàn bộ 1–109

| Nhóm | Mục gốc | Công việc phải giao đủ | Kết quả cần có |
|---|---|---|---|
| A | 1–21 | Tích hợp source/tài liệu, Chromium desktop, sandbox, worker/presentation, quota, khôi phục, Linux/systemd và môi trường macOS/VM | Source và packaging nhất quán theo nền tảng; giữ riêng kết quả build/runtime chưa đủ |
| B | 22–36 | Core GB/GBA/NES/SNES/DS/Mega Drive/N64/PS1/PSP; N64 snapshot/save/GPU/audio; PSP font; core/frontend protocol | Core registry, artifact và protocol khớp nhau; provenance và giới hạn từng core rõ ràng |
| C | 37–45 | Hiệu chỉnh joystick, touch native, input mobile, controller đặc thù, GPU và vòng đời video/audio/input | UI → cấu hình → runtime hoạt động trên source; các ca thiết bị thật để hàng đợi qualification |
| D | 46–52 | Backup profile, sửa DB, tái dựng metadata, quota/tham chiếu, phục hồi thủ công và archive/storage fault | Không mất dữ liệu khi sửa/khôi phục; kế thừa bằng chứng 50–52 và ghi phạm vi còn thiếu |
| E | 53–60 | Capture Linux/macOS, virtual gamepad, service encode, launcher ownership, credential, signaling/TURN và Remote Play thật | Đường sản phẩm tích hợp capture/audio → encode → transport xác thực → decode/present; revoke dừng đúng owner |
| F | 61–66 | Android core/runtime, Remote Play, đồng bộ nền/document provider, branding/l10n/a11y, APK/AAB, thiết bị thật | Luồng ứng dụng nối JNI/Rust/Chromium; không nhận source-only là APK/device pass |
| G | 67–72 | iOS không JIT, Remote Play, background/File Provider, UI/icon/l10n/a11y, Xcode và thiết bị | Caller/lifecycle/native APIs đầy đủ; build/sign/device qualification theo quyền riêng |
| H | 73–76 | Dependency ngoại tuyến, cài/cập nhật/phục hồi PWA, quota/cache và browser matrix | Đường offline/update/recovery có xử lý lỗi và dữ liệu phiên cũ; thiết bị/browser evidence ghi riêng |
| I | 77–83 | Tài khoản/credential, artifact crypto, sync HTTPS, mã hóa phân đoạn, cancel/reconcile, quota/orphan/conflict/revoke | Source client/server tương thích, truyền dữ liệu có giới hạn bộ nhớ và phục hồi giao dịch |
| J | 84–87 | Benchmark registry, lựa chọn runtime, instrumentation presentation, benchmark thực tế | Bản ghi gắn đúng game/core/runtime/device/config; không bịa FPS/latency từ mock |
| K | 88–95 | Installer UI, repair, uninstall, Windows bootstrap, Linux packages, macOS updates, quota/cleanup/rollback | Installer/updater có giao dịch và khôi phục; artifact ký và qualification thực tế còn là gate |
| L | 96–104 | License/provenance, corresponding source, SBOM, CVE, trust root, signing, store/privacy, security intake và audit độc lập | Hồ sơ gắn đúng binary; secrets không vào repo; audit độc lập không thay bằng tự kiểm tra |
| M | 105–109 | Fixture Windows, ma trận app, clean-clone CI, phản hồi thật và đánh giá phát hành | Bằng chứng chạy thật khi được cho phép; release decision theo từng mục gốc |

Các nhóm là cách tổ chức, không thay thứ tự gốc hoặc tự cấp quyền chạy song song. Đường tiếp tục đầu tiên: nhóm E, từ mục 53 còn thiếu ở source, sau đó 54–60; không viết lại các thành phần đã có chỉ vì chưa được kiểm thử. Nhóm A–D cần đối soát owner với các chat desktop/core hiện hữu trước khi tiếp quản file; những việc còn thiếu vẫn phải được hoàn tất trước nghiệm thu toàn dự án.

## 5. Gói triển khai đầu tiên: Remote Play native

### 5.1 Những phần đã có source

- `platform/stream-input/src/video_winsock.rs`: binding local address/interface, active software timestamp, packet-info, WSARecvMsg và chuyển QPC thành tuổi gói. Win32 unsafe nằm trong crate native; broker giữ `forbid(unsafe_code)`.
- `platform/broker/src/stream_video_ingress.rs`: `NativeSrtpReceiver`, UDP ingress, constructor Windows và transport slot có giữ ownership khi attachment thất bại.
- `platform/broker/src/stream_video_registry.rs`: opaque document lifetime và receive registry.
- `platform/broker/src/stream_video_session.rs`, `stream_video_authority.rs`, `stream_video_codec.rs`, `stream_video_receive.rs`: session/authority, decoder admission, hàng đợi và cleanup có deadline.
- `platform/streaming/src/native_h264_*.rs`: parsing SPS/PPS/slice/AU, reference/POC state và commit sau decoder completion. Không được coi các parser này là decoder hoặc là chứng minh stream hợp lệ toàn bộ.
- `platform/streaming/src/native_ice.rs`: admission TURN credential; chưa phải ICE agent.
- Credential/pairing native/mobile đã có nhiều caller và journal. Xem `docs/NATIVE_PAIRING_CREDENTIALS.md`, `docs/LINUX_OWNED_CAPTURE.md`, `docs/MACOS_OWNED_CAPTURE.md`, `docs/VIRTUAL_GAMEPAD_OWNERSHIP.md`, `docs/WINDOWS_LAUNCHER_OWNERSHIP.md`, `docs/NATIVE_VIDEO_SERVICE.md`, `docs/NATIVE_VIDEO_RECEIVE_BRIDGE.md`.

Checkpoint này chưa build/test. Không được mặc định compile được hoặc dùng nó làm artifact đã chấp nhận.

### 5.2 Công việc phải triển khai tiếp

1. Đọc đúng thiếu sót của capture Linux/macOS (53–54), hoàn thiện caller/ownership/audio và đường truyền frame còn thiếu ở source. Không tự dùng SSH hay chạy build để mở đầu batch.
2. Hoàn thiện virtual gamepad (55): tích hợp backend, giới hạn slot, ownership, hotplug/disconnect và release input. Driver hoặc phần cứng chưa được cấp cho phiên mới phải ghi là phụ thuộc.
3. Hoàn thiện service capture/encode (56): packaging, supervision, admission producer, encoder capability thật, codec/profile/level và âm thanh đồng bộ. Không thay service bằng mailbox primitives.
4. Hoàn thiện ICE candidate gathering/connectivity/nomination, selected socket + interface, TURN allocation/refresh/expiry và consent/reconnect. Không nhận peer do renderer gửi là bằng chứng đã chọn ICE.
5. Nối DTLS-SRTP bằng implementation đã được duyệt; kiểm tra fingerprint/identity gắn grant/pair/session/generation và hướng khóa exporter. Không tự viết primitive crypto, không plaintext fallback, không đẩy khóa vào IPC renderer/log.
6. Cài backend thực tế cho `NativeSrtpReceiver`: authentication/decryption/replay trước RTP, SSRC/codec thương lượng và SRTCP/DTLS/STUN demux đúng. Giữ receipt ingress gốc; ciphertext/packet header không tự cấp quyền.
7. Nối caller thực tế trong broker/native document: negotiation → open session → selected socket ingress → registry → decoder → native presentation/audio. Giữ owner/document/focus/capture authority ở các ranh giới; không chỉ thêm trait/factory mà không có caller.
8. Chốt launcher child-process ownership (57) và credential producer stop/crash reconciliation (58). Revoke phải chặn producer/consumer, dừng input/gamepad/audio, đóng transport/helper và ghi nợ cleanup nếu chưa xác nhận.
9. Chuẩn bị deployment source/config mẫu cho signaling/relay/TURN (59) nhưng không deploy/bật production; ghi rõ domain, TLS, secrets, chi phí/vận hành cần chủ dự án cung cấp.
10. Chuẩn bị ca mạng thật/reconnect/revoke/capture lỗi (60), giữ chưa kiểm chứng cho đến khi được phép chạy.

Không khép mục 56/60 khi vẫn chỉ có FIFO hoặc subset codec không đáp ứng toàn bộ profile đã duyệt. Các giới hạn reorder, B-frame, timestamps, NIC và encoder phải được xử lý theo capability thương lượng thật hoặc ghi đúng phần còn thiếu; không giảm scope sản phẩm một cách ngầm định.

## 6. Cách thực hiện từng batch

1. Nhận đúng mục gốc, ghi owner và những file được sửa. Đọc source liên quan và tài liệu trạng thái; không quét toàn máy hoặc lặp lại inventory không phục vụ thay đổi.
2. Ghi trước đường gọi thực tế cần đạt, đầu vào/đầu ra, quyền sở hữu tài nguyên và rollback. Chọn batch đủ để nối một luồng sử dụng được thay vì hàng loạt interface rời rạc.
3. Viết implementation và nối caller/UI/package tương ứng. Xử lý cancel, timeout, crash/restart, revoke và lỗi I/O trong cùng batch nếu đường đó sở hữu tài nguyên.
4. Trong giai đoạn hoãn build/test: chỉ rà diff/source cần thiết; ghi rõ `SOURCE_READY_UNVERIFIED`. Chuẩn bị đúng lệnh và ca acceptance để chạy sau; không tạo báo cáo PASS giả.
5. Commit chỉ file của batch trên nhánh bàn giao. Không dùng `git add .` hoặc `git commit -a` trong checkout có nhiều owner. Không force push/rebase thay lịch sử chat khác. Merge/push vào nhánh chính phải theo phân công tích hợp, không coi quyền sửa local là quyền phát hành.
6. Báo cáo theo mẫu, cập nhật ledger và thiếu sót. Không đóng một mục chỉ vì tool báo SUCCESS hoặc không còn TODO.
7. Tiếp tục batch sau trong phần được cấp quyền; khi cần quyền/tài nguyên mới, ghi request cụ thể và làm phần độc lập còn lại.

## 7. Tiêu chí nghiệm thu và hàng đợi build/test

Mỗi mục cần đủ source thực thi, caller, packaging/config có liên quan, đường lỗi/cleanup và tài liệu đúng. Với mục có yêu cầu runtime/build/thiết bị, source hoàn chỉnh vẫn chưa đủ để đánh dấu `[x]`.

Khi chủ dự án cho phép mở lại build/test, dùng queue có thứ tự phụ thuộc:

1. Compile/check đúng crate/target đã thay đổi; xử lý lỗi compile trước test tích hợp.
2. Test có mục tiêu cho contract/ownership/protocol/crypto/cleanup đã sửa, dùng fixture hợp lệ; tránh kiểm tra trùng lặp toàn repo vô cớ.
3. Luồng đầu cuối trong app Windows, rồi platform Android/iOS/Linux/macOS phù hợp batch và thiết bị đã được cấp.
4. Remote Play mạng thật: pair → consent → capture/audio → receive/present → input → reconnect/revoke; giữ log không chứa bí mật và số liệu latency/cleanup thật.
5. Installer/update/repair/rollback và clean-clone CI từ đúng commit/artifact.
6. License, corresponding source, SBOM, ký mã, store/privacy và audit độc lập trước quyết định phát hành.

Mỗi kết quả phải ghi commit, target, môi trường, lệnh, exit/result, artifact/log và giới hạn. Build debug không chứng minh gói release đã ký; Windows pass không chứng minh mobile/Linux/macOS pass. Trạng thái hạ tầng live, tài khoản, máy từ xa và job cũ phải được xác nhận tại lúc thực hiện, không suy từ tài liệu này.

## 8. Điểm kết thúc bàn giao

Antigravity đã nhận việc khi có cuộc trò chuyện App xác định, workspace đúng, prompt đã gửi và phản hồi đầu tiên ghi batch/owner/next action. Hiện bộ tài liệu chỉ chuẩn bị các thông tin đó; không chứa session App hay xác nhận giao nhận.

Toàn dự án hoàn thành khi từng mục 1–109 đã được xử lý đúng phạm vi, bằng chứng đạt yêu cầu từng mục và chủ dự án chốt mức sẵn sàng phát hành ở mục 109. Không dùng một tỷ lệ phần trăm tổng hợp để thay điều kiện này.
