//! Estado visual do seletor de cores do FeatherDocs.
//! O widget e a conversão HSV são compartilhados; a edição pertence ao motor Docs.
use feather_ui::seletor_cor::{self, Evento};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlvoCor {
    Texto,
    Realce,
    Fundo,
    Pagina,
}

#[derive(Clone, Debug)]
pub struct EstadoCor {
    pub alvo: Option<AlvoCor>,
    pub atual: String,
    pub texto_hex: String,
    pub hsva: [f32; 4],
}

impl Default for EstadoCor {
    fn default() -> Self {
        Self {
            alvo: None,
            atual: String::new(),
            texto_hex: String::new(),
            hsva: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl EstadoCor {
    /// Abre o seletor com a cor efetiva da seleção, sem modificar o documento.
    pub fn abrir(&mut self, alvo: AlvoCor, cor: &str) {
        self.alvo = Some(alvo);
        self.atual = cor.to_owned();
        self.texto_hex = cor.to_owned();
        self.hsva = seletor_cor::rgba_para_hsva(
            seletor_cor::rgba_de(cor),
            self.hsva[0],
        );
    }

    fn definir(&mut self, cor: String) {
        self.hsva = seletor_cor::rgba_para_hsva(
            seletor_cor::rgba_de(&cor),
            self.hsva[0],
        );
        self.texto_hex = cor.clone();
        self.atual = cor;
    }

    fn gerar_da_hsva(&mut self) -> String {
        let [h, s, v, a] = self.hsva;
        let rgb = seletor_cor::hsv_para_rgb(h, s, v);
        let hex = seletor_cor::hex_de([
            rgb[0],
            rgb[1],
            rgb[2],
            (a * 255.0).round() as u8,
        ]);
        self.definir(hex.clone());
        hex
    }

    /// Só devolve alterações válidas, para que entradas parciais não poluam o undo.
    /// "Fechar" apenas fecha a UI: não altera a seleção nem a formatação.
    pub fn evento(&mut self, evento: Evento) -> Option<(AlvoCor, String)> {
        let alvo = self.alvo?;
        let nova = match evento {
            Evento::Fechar => {
                self.alvo = None;
                return None;
            }
            Evento::Escolhida(cor) => {
                if !cor.is_empty() && !seletor_cor::hex_valido(&cor) {
                    return None;
                }
                self.definir(cor.clone());
                cor
            }
            Evento::TextoHex(cor) => {
                self.texto_hex = cor.clone();
                if !seletor_cor::hex_valido(&cor) {
                    return None;
                }
                self.definir(cor.clone());
                cor
            }
            Evento::Canal(canal, valor) => {
                if canal >= 4 {
                    return None;
                }
                let Ok(valor) = valor.parse::<u8>() else {
                    return None;
                };
                let mut rgba = seletor_cor::rgba_de(&self.atual);
                rgba[canal] = valor;
                let cor = seletor_cor::hex_de(rgba);
                self.definir(cor.clone());
                cor
            }
            Evento::SaturacaoBrilho(s, v) => {
                if !s.is_finite() || !v.is_finite() {
                    return None;
                }
                self.hsva[1] = s.clamp(0.0, 1.0);
                self.hsva[2] = v.clamp(0.0, 1.0);
                self.gerar_da_hsva()
            }
            Evento::Matiz(h) => {
                if !h.is_finite() {
                    return None;
                }
                self.hsva[0] = h.rem_euclid(360.0);
                self.gerar_da_hsva()
            }
            Evento::Alfa(a) => {
                if !a.is_finite() {
                    return None;
                }
                self.hsva[3] = a.clamp(0.0, 1.0);
                self.gerar_da_hsva()
            }
        };
        Some((alvo, nova))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn seletor_apenas_emite_cores_validas_e_fecha_sem_editar() {
        let mut estado = EstadoCor::default();
        assert!(estado.evento(Evento::Escolhida("#ff0000".into())).is_none());
        estado.abrir(AlvoCor::Texto, "");
        assert!(estado.evento(Evento::TextoHex("#ff".into())).is_none());
        assert_eq!(estado.texto_hex, "#ff");
        assert_eq!(
            estado.evento(Evento::Escolhida("#112233".into())),
            Some((AlvoCor::Texto, "#112233".into()))
        );
        assert_eq!(
            estado.evento(Evento::Canal(3, "128".into())),
            Some((AlvoCor::Texto, "#11223380".into()))
        );
        assert!(estado.evento(Evento::Canal(4, "10".into())).is_none());
        assert!(estado.evento(Evento::Fechar).is_none());
        assert_eq!(estado.alvo, None);
    }

    #[test]
    fn hsv_e_alfa_geram_cor_compativel_com_motor() {
        let mut estado = EstadoCor::default();
        estado.abrir(AlvoCor::Realce, "#ff0000");
        assert_eq!(
            estado.evento(Evento::Matiz(120.0)),
            Some((AlvoCor::Realce, "#00ff00".into()))
        );
        assert_eq!(
            estado.evento(Evento::Alfa(128.0 / 255.0)),
            Some((AlvoCor::Realce, "#00ff0080".into()))
        );
        assert!(estado.evento(Evento::Matiz(f32::NAN)).is_none());
        assert_eq!(estado.atual, "#00ff0080");
    }
}
