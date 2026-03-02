# Sniper Scraper 3.0 (Precision Mode)

Auto-Crawler projesinin evrimleşmiş, anti-bot sistemlerine (Cloudflare vb.) karşı %100 dayanıklı ve kullanıcı kontrollü yeni sürümü. Artık "Otomatik Kazıma" yerine, insanın navigasyon gücü ile makinenin kayıt hızını birleştiren bir **Keskin Nişancı (Sniper)** modunda çalışır.

## 🚀 Öne Çıkan Özellikler

- **Anti-Bot Precision:** Cloudflare ve Captcha engellerini, gerçek tarayıcınızda (Chrome/Chromium) manuel olarak geçip programa sadece "Kaydet" emri vererek aşarsınız.
- **Multi-Tab Support:** Belirlediğiniz port üzerinden tarayıcıdaki tüm açık sekmeleri görür, istediğiniz sekmeyi seçip anlık HTML kopyasını alırsınız.
- **UTF-8 Full Support:** Türkçe, Korece ve diğer tüm dillerde karakter bozulması olmadan kayıt ve görüntüleme.
- **Sistem Profili Entegrasyonu:** Kendi tarayıcı profilinizi (çerezler, şifreler) otomatik tespit edip kullanabilme.
- **Orphan Process Protection:** Program kapandığında tarayıcı ve tüm sekmelerini otomatik olarak sonlandırır.
- **AI Translation:** Kazınan ham HTML dosyalarını toplu olarak Google Gemini API üzerinden profesyonel kalitede Türkçeye çevirme.

## 🛠 Kurulum ve Kullanım (Sniper Modu)

### 1. Adım: Tarayıcı Hazırlığı
Programın tarayıcınıza bağlanabilmesi için Chrome/Chromium'u Remote Debugging portu ile başlatmalısınız:
```bash
google-chrome --remote-debugging-port=9222
```

### 2. Adım: Programı Başlatın
```bash
cargo run
```

### 3. Adım: Operasyon
1. **Output Dir:** Kayıt edilecek ana klasörü seçin.
2. **Step 1:** URL girip `LAUNCH BROWSER` deyin (Veya hali hazırda açıksa sadece portu kontrol edin).
3. **Step 2:** Tarayıcıda romanın bölümünü açın. GUI'de `REFRESH LIST` diyerek sekmeyi seçin.
4. **Step 3:** `CAPTURE TARGET PAGE` butonuna basın. 
   - *Sonuç:* `site.com/bolum.adi.html` şeklinde UTF-8 HTML dosyanız hazır!

## 🌐 AI Çeviri (Translate Tab)
1. **Raw Folder:** Kazınan HTML'lerin olduğu klasörü seçin.
2. **Output Folder:** Çevirilerin kaydedileceği yeri seçin.
3. **API Key:** Gemini API anahtarınızı girin.
4. **Start:** Program tüm dosyaları sırayla okur ve AI ile yerelleştirir.

## 📜 Loglama
Program her çalışma için `logs/` klasörü altında iki adet log tutar:
- `logs/{TIME}.log`: Uygulama adımları ve başarı durumları.
- `logs/chrome.{TIME}.log`: Tarayıcıdan gelen içsel mesajlar ve hatalar.

---
**Geliştirici Notu:** Bu araç eğitim amaçlı geliştirilmiştir. Kullanırken ilgili sitelerin kullanım koşullarına uyunuz.
