//! Türkçe metin yardımcıları: arama için katlama, Diyanet adlarını düzgün yazma.

/// Aramada kullanılan katlama: küçük harf, Türkçe karakterler ASCII'ye, harf/rakam
/// dışı her şey atılır. `"KÖPRÜBAŞI (T)"` → `"koprubasit"`.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let m = match c {
            'İ' | 'I' | 'ı' | 'i' | 'Î' | 'î' => 'i',
            'Ş' | 'ş' => 's',
            'Ğ' | 'ğ' => 'g',
            'Ü' | 'ü' | 'Û' | 'û' => 'u',
            'Ö' | 'ö' => 'o',
            'Ç' | 'ç' => 'c',
            'Â' | 'â' | 'Ä' | 'ä' => 'a',
            'É' | 'é' | 'È' | 'è' | 'Ë' | 'ë' => 'e',
            c if c.is_alphanumeric() => {
                out.extend(c.to_lowercase());
                continue;
            }
            _ => continue,
        };
        out.push(m);
    }
    out
}

fn lower(c: char, turkish: bool) -> String {
    match c {
        'I' if turkish => "ı".to_string(),
        'İ' => "i".to_string(),
        c => c.to_lowercase().collect(),
    }
}

fn upper(c: char, turkish: bool) -> String {
    match c {
        'i' if turkish => "İ".to_string(),
        'ı' => "I".to_string(),
        c => c.to_uppercase().collect(),
    }
}

/// Diyanet'in BÜYÜK HARF adlarını başlık biçimine çevirir:
/// `"KÖPRÜBAŞI (T)"` → `"Köprübaşı (T)"`, `"İSTANBUL"` → `"İstanbul"`.
/// `turkish`: Türkçe I/İ kuralları (yalnızca Türkiye adları; `"BERLIN"` → `"Berlin"`).
/// Parantez içindeki kısa kısaltmalar olduğu gibi kalır.
pub fn title(s: &str, turkish: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    let mut in_paren = false;
    let mut paren_buf = String::new();
    for c in s.chars() {
        if in_paren {
            paren_buf.push(c);
            if c == ')' {
                in_paren = false;
                let inner = &paren_buf[1..paren_buf.len() - 1];
                if inner.chars().filter(|c| c.is_alphabetic()).count() <= 3 {
                    out.push_str(&paren_buf);
                } else {
                    out.push('(');
                    out.push_str(&title(inner, turkish));
                    out.push(')');
                }
                paren_buf.clear();
                start = true;
            }
            continue;
        }
        if c == '(' {
            in_paren = true;
            paren_buf.push(c);
            continue;
        }
        if c.is_alphabetic() {
            if start {
                let l = lower(c, turkish).chars().next().unwrap_or(c);
                out.push_str(&upper(l, turkish));
            } else {
                out.push_str(&lower(c, turkish));
            }
            start = false;
        } else {
            out.push(c);
            start = c == ' ' || c == '-' || c == '.' || c == '/';
        }
    }
    out.push_str(&paren_buf);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_turkish() {
        assert_eq!(fold("KÖPRÜBAŞI (T)"), "koprubasit");
        assert_eq!(fold("İstanbul"), "istanbul");
        assert_eq!(fold("ISPARTA"), "isparta");
        assert_eq!(fold("Çağlayancerit"), "caglayancerit");
    }

    #[test]
    fn titles() {
        assert_eq!(title("İSTANBUL", true), "İstanbul");
        assert_eq!(title("KÖPRÜBAŞI (T)", true), "Köprübaşı (T)");
        assert_eq!(title("ISPARTA", true), "Isparta");
        assert_eq!(title("M.EREĞLİSİ", true), "M.Ereğlisi");
        assert_eq!(title("AKKOL (Ekibastuz)", false), "Akkol (Ekibastuz)");
        assert_eq!(title("BADEN WURTTEMBERG", false), "Baden Wurttemberg");
        assert_eq!(title("BERLIN", false), "Berlin");
    }
}
