SRC_DIR="src"
OUTPUT="SRC.md"

# Çıktı dosyasını sıfırla
: > "$OUTPUT"

# src altındaki tüm dosyaları tara
find "$SRC_DIR" -type f | while read -r file; do
    # Dosya metin mi kontrol et
    if file "$file" | grep -q "text"; then
        echo "## $file" >> "$OUTPUT"
        echo '```' >> "$OUTPUT"
        cat "$file" >> "$OUTPUT"
        echo >> "$OUTPUT"
        echo '```' >> "$OUTPUT"
        echo >> "$OUTPUT"
    fi
done

echo "Tamamlandı: $OUTPUT oluşturuldu."
