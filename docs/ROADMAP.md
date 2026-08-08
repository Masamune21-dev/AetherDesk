# Development Roadmap

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Fase Pengembangan

```
2027                              2028                              2029
Q1      Q2      Q3      Q4      Q1      Q2      Q3      Q4      Q1      Q2
├───────┼───────┼───────┼───────┼───────┼───────┼───────┼───────┼───────┤

Fase 1: Foundation (MVP)
├═══════════════════════════╗
│ Core Protocol & Agent     ║ (Q1-Q2 2027)
│ Viewer (Tauri)            ║
│ WebRTC P2P + Relay        ║
│ Win + macOS               ║
╚═══════════════════════════╝
         ▼
         Fase 2: Enterprise
         ├═══════════════════════════╗
         │ SSO/MFA/RBAC             ║ (Q3-Q4 2027)
         │ Multi-tenant             ║
         │ Session Recording        ║
         │ Linux + Android Viewer   ║
         │ Remote Terminal          ║
         ╚═══════════════════════════╝
                  ▼
                  Fase 3: Scale
                  ├═══════════════════════════╗
                  │ Multi-region              ║ (Q1-Q2 2028)
                  │ Plugin SDK & Webhooks     ║
                  │ H.265 / AV1              ║
                  │ iOS + Web Viewer          ║
                  │ SCIM / LDAP / ABAC       ║
                  ╚═══════════════════════════╝
                           ▼
                           Fase 4: Intelligence
                           ├═══════════════════════════╗
                           │ AI Troubleshooting        ║ (Q3 2028 - Q2 2029)
                           │ USB/Smart Card            ║
                           │ Voice/Video/Whiteboard    ║
                           │ Remote Printing           ║
                           ╚═══════════════════════════╝
```

---

## 2. Detil Milestones

### Fase 1: Foundation (Bulan 1 - 6)
- **Bulan 1-2**: Desain skema database, pembuatan API server dasar (Axum), registrasi agent.
- **Bulan 3-4**: Screen capture DXGI & ScreenCaptureKit, hook keyboard/mouse, basic streaming H.264 WebRTC.
- **Bulan 5-6**: Implementasi Tauri viewer dasar, file transfer sederhana, mTLS auth, dan rilis MVP.

### Fase 2: Enterprise (Bulan 7 - 12)
- **Bulan 7-8**: SSO (SAML/OIDC), MFA (TOTP/WebAuthn), RBAC granular, organisasi & multi-tenancy.
- **Bulan 9-10**: Session recording terenkripsi di server, audit logs immutable, Linux agent/viewer support.
- **Bulan 11-12**: Remote terminal (PowerShell/CMD/Bash), inventaris hardware/software, system health monitoring.

### Fase 3: Scale (Bulan 13 - 18)
- **Bulan 13-14**: Deployment TURN/Relay cluster multi-region, DNS routing latency-based, optimization AV1.
- **Bulan 15-16**: Android & iOS viewer apps, Web Assembly (Wasm) zero-install web viewer.
- **Bulan 17-18**: SCIM user provisioning, integrasi LDAP/Active Directory, ABAC policy controller.

### Fase 4: Intelligence & Advanced Features (Bulan 19 - 24)
- **Bulan 19-20**: AI-assisted troubleshooting (menganalisis crash log dan performa secara otomatis).
- **Bulan 21-22**: Voice/video call interkom, annotation overlay, laser pointer, whiteboard untuk training.
- **Bulan 23-24**: USB redirection, Smart Card passthrough, remote printing driver, sertifikasi SOC 2 Type II.
