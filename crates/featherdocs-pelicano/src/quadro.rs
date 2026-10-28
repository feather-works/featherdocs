//! Decodifica o contrato binário do motor; não recalcula o layout do documento.
#[derive(Clone, Debug)]
pub enum Operacao {
    Texto {
        x: f32,
        y: f32,
        tamanho: f32,
        estilo: u8,
        cor: u32,
        texto: String,
        angulo: Option<f32>,
    },
    Retangulo {
        rect: [f32; 4],
        cor: u32,
    },
    Traco {
        pontos: [f32; 5],
        cor: u32,
    },
    Imagem {
        rect: [f32; 4],
        origem: String,
    },
}
#[derive(Clone, Debug)]
pub struct Quadro {
    pub largura: f32,
    pub altura: f32,
    pub operacoes: Vec<Operacao>,
}
struct Leitor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl Leitor<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let fim = self.pos.checked_add(N).ok_or("Quadro inválido.")?;
        let b = self.bytes.get(self.pos..fim).ok_or("Quadro incompleto.")?;
        self.pos = fim;
        b.try_into().map_err(|_| "Quadro inválido.".into())
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.bytes::<1>()?[0])
    }
    fn numero(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }
    fn f(&mut self) -> Result<f32, String> {
        let n = f32::from_le_bytes(self.bytes()?);
        if n.is_finite() {
            Ok(n)
        } else {
            Err("Coordenada inválida no quadro.".into())
        }
    }
    fn valores<const N: usize>(&mut self) -> Result<[f32; N], String> {
        let mut v = [0.; N];
        for n in &mut v {
            *n = self.f()?;
        }
        Ok(v)
    }
    fn texto(&mut self) -> Result<String, String> {
        let n = usize::from(u16::from_le_bytes(self.bytes()?));
        self.utf8(n)
    }
    fn texto_longo(&mut self) -> Result<String, String> {
        let n = u32::from_le_bytes(self.bytes()?) as usize;
        self.utf8(n)
    }
    fn utf8(&mut self, n: usize) -> Result<String, String> {
        let fim = self.pos.checked_add(n).ok_or("Quadro inválido.")?;
        let b = self.bytes.get(self.pos..fim).ok_or("Texto incompleto.")?;
        self.pos = fim;
        String::from_utf8(b.to_vec()).map_err(|_| "Texto inválido no quadro.".into())
    }
}
impl Quadro {
    pub fn ler(bytes: &[u8]) -> Result<Self, String> {
        let mut l = Leitor { bytes, pos: 0 };
        if l.byte()? != 1 {
            return Err("Quadro sem conteúdo novo.".into());
        }
        let _versao = l.bytes::<8>()?;
        let [largura, altura] = l.valores()?;
        if largura <= 0. || altura <= 0. {
            return Err("Dimensões inválidas.".into());
        }
        let n = l.numero()? as usize;
        if n > bytes.len() {
            return Err("Quantidade inválida de operações.".into());
        }
        let mut operacoes = Vec::new();
        for _ in 0..n {
            operacoes.push(match l.byte()? {
                tipo @ (1 | 5) => {
                    let [x, y, tamanho] = l.valores()?;
                    let angulo = if tipo == 5 { Some(l.f()?) } else { None };
                    let estilo = l.byte()?;
                    let cor = l.numero()?;
                    let texto = l.texto()?;
                    Operacao::Texto {
                        x,
                        y,
                        tamanho,
                        estilo,
                        cor,
                        texto,
                        angulo,
                    }
                }
                2 => Operacao::Retangulo {
                    rect: l.valores()?,
                    cor: l.numero()?,
                },
                3 => Operacao::Traco {
                    pontos: l.valores()?,
                    cor: l.numero()?,
                },
                4 => Operacao::Imagem {
                    rect: l.valores()?,
                    origem: l.texto_longo()?,
                },
                _ => return Err("Operação desconhecida no quadro.".into()),
            });
        }
        Ok(Self {
            largura,
            altura,
            operacoes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quadro_do_motor_e_truncamentos() {
        let mut s = featherdocs_core::sessao::Sessao::default();
        s.inserir_texto("Olá, edição nativa!");
        let bytes = s.quadro(0, u64::MAX).unwrap();
        let q = Quadro::ler(&bytes).unwrap();
        assert!(q
            .operacoes
            .iter()
            .any(|o| matches!(o, Operacao::Texto { texto, .. } if texto.contains("Olá"))));
        for fim in 0..bytes.len() {
            assert!(Quadro::ler(&bytes[..fim]).is_err());
        }
    }
}
