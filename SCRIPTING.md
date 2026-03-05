# SCRIPTING Guide

Bu doküman iki bölümden oluşur:
1. **Tutorial**: Scripting tabını adım adım nasıl kullanacağınız
2. **Scripting API**: Rhai tarafında kullanılabilen API yüzeyi ve davranışları

---

## 1) Tutorial

### 1.1 Scripting tabına hızlı başlangıç
1. Uygulamayı başlatın ve browser instance'ını açın.
2. `Ops` panelinden hedef tabı seçin.
3. `Scripting` sekmesine geçin.
4. `Execution Target` alanında hedefi doğrulayın.
5. Scripti yazın veya `Import .json` ile paket açın.
6. Önce `Check`, sonra `Execute` çalıştırın.

Not:
- `Check` kodu **çalıştırmaz**, sadece derleme/lint doğrulaması yapar.
- Script logları `Scripting` panelinde değil, `System Telemetry` panelindedir.

### 1.2 Script package (.json) formatı
Scripting paneli şu paketi kullanır:

```json
{
  "version": 1,
  "name": "example_script",
  "description": "demo",
  "created_at": 1741140000,
  "updated_at": 1741143600,
  "entry": "main",
  "code": "fn main() { log(\"hello\"); }",
  "tags": ["demo"]
}
```

### 1.3 İlk script (navigate + capture)
```rhai
fn main() {
    let tab = Tab("https://example.com");
    tab.wait_for_ms(1200);
    tab.capture.html();
    log("HTML capture tamamlandi");
}
```

### 1.4 Mevcut taba bağlanma
```rhai
fn main() {
    let tab = TabCatch();
    tab.navigate("https://example.org");
    log("Secili tab yeniden yonlendirildi");
}
```

### 1.5 Element bazlı işlem
```rhai
fn main() {
    let tab = Tab("https://duckduckgo.com");
    tab.wait_for_ms(1000);

    let input = tab.find_el("input[name='q']");
    input.type("sniper studio scripting");

    let btn = tab.find_el("button[type='submit']");
    btn.click();
}
```

### 1.6 Automation DSL'i script içinden çağırma
```rhai
fn main() {
    let tab = TabCatch();

    let dsl = `{
      "dsl_version": 1,
      "metadata": null,
      "functions": {},
      "steps": [
        { "type": "Wait", "seconds": 1 },
        { "type": "ScrollBottom" }
      ]
    }`;

    tab.run_automation_json(dsl);
    log("Automation DSL script icinden calistirildi");
}
```

### 1.7 Güvenli dosya yazımı (output_dir scope)
Script, sadece uygulamanın `output_dir` ağacı içinde dosya yazabilir.

```rhai
fn main() {
    fs_mkdir_all("script_outputs/run1");
    fs_write_text("script_outputs/run1/result.txt", "Ilk satir");
    fs_append_text("script_outputs/run1/result.txt", "Ikinci satir");
}
```

### 1.8 Hata ayıklama önerisi
- Önce `Check` yapın.
- Ardından küçük adımlarla `Execute` edin.
- Hata/çıktı için `System Telemetry` paneline bakın.
- Browser console kaynaklı satırlar `[CHROME]` olarak görünür.

---

## 2) Scripting API Reference

Bu bölüm mevcut backend implementasyonuna göre hazırlanmıştır.

### 2.1 Global fonksiyonlar
- `log(message: string)`
- `exit(message: string)`
- `fs_write_text(rel_path: string, content: string)`
- `fs_append_text(rel_path: string, content: string)`
- `fs_mkdir_all(rel_dir: string)`
- `fs_exists(rel_path: string) -> bool` *(şu an temel stub davranışındadır)*

### 2.2 Tab oluşturma/bağlama
- `Tab()` -> yeni boş tab
- `Tab(url: string)` -> URL ile yeni tab
- `TabNew()` -> yeni boş tab (alias)
- `TabCatch()` -> UI’de seçili taba bağlanır

### 2.3 Tab metotları
- `tab.navigate(url)`
- `tab.wait_for_ms(ms)`
- `tab.screenshot()`
- `tab.screenshot(name)`
- `tab.find_el(selector) -> ElementRef`
- `tab.run_automation_json(dsl_json)`

### 2.4 ElementRef metotları
- `el.click()`
- `el.type(value)`

### 2.5 Servis objeleri
#### Capture
- `tab.capture.html()`
- `tab.capture.mirror()`
- `tab.capture.complete()`

#### Console
- `tab.console.inject(js_code)`
- `tab.console.logs() -> Array` *(şu an boş dönüş/stub)*

#### Network
- `tab.network.start()`
- `tab.network.stop()`

#### Cookies
- `tab.cookies.set(name, value, overwrite)`
- `tab.cookies.delete(name, domain)`
- `tab.cookies.get_all() -> Map` *(şu an boş dönüş/stub)*

### 2.6 Check butonunun doğruladıkları
- Rhai compile hataları
- `entry` alanının dolu olması
- `entry` isimli fonksiyonun kod içinde bulunması
- Sık yapılan sözdizimi hataları için temel uyarılar (örn. Rust raw string kullanımı)

### 2.7 Cancel/Stop davranışı
- `Stop` cooperative cancel gönderir.
- Uzun/harici tek bir adım, adım sonu noktada durabilir.
- Sistem Telemetry'de stop kaynaklı çıktı/hata görülebilir.

### 2.8 Log ve dosya davranışı
- Script logları: `System Telemetry`
- Program log dosyası: `session_<timestamp>.log`
- Chrome log dosyası: `chrome_session_<timestamp>.log`

### 2.9 Bilinen sınırlamalar
- `Tab.catch()` adı yerine şu an `TabCatch()` aktif.
- `console.logs()` ve `cookies.get_all()` tam veri dönüşüne henüz genişletilmedi.
- İleri düzey query/filter zinciri (`findEl().filter(...)`) bu sürümde sınırlı.

---

## Katkı Notu (Developers)
Yeni API eklerken önerilen yaklaşım:
1. Önce `core/scripting/engine.rs` içinde action tanımı ekleyin.
2. Rhai registration fonksiyonunu ekleyin.
3. Action execution branch’ini yazın.
4. `Check` için gerekli lint/diagnostic kuralını güncelleyin.
5. `System Telemetry` akışında görünürlüğü doğrulayın.
6. `cargo check` çalıştırın.
