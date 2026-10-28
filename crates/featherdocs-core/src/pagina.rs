//! O que vale para a página inteira e não é bloco: cabeçalho, rodapé, número de página,
//! cor do papel, marca d'água, borda da página e as marcas de formatação (¶ ·) à vista.
//! O desenho disso tudo está em `quadro.rs`.

use crate::fonte::ler_cor;
use crate::modelo::{Extras, Faixas, NumeracaoDeLinhas, Vertical};
use crate::sessao::{Mudanca, Sessao};

/// Os lugares de um número de página: `topo_esquerda`, `topo_centro`… `base_direita`.
fn lugar(onde: &str) -> Result<(bool, u8), String> {
    let (topo, resto) = if let Some(r) = onde.strip_prefix("topo_") {
        (true, r)
    } else if let Some(r) = onde.strip_prefix("base_") {
        (false, r)
    } else {
        return Err(format!("Lugar de número de página desconhecido: \"{onde}\"."));
    };
    let lado = match resto {
        "esquerda" => 0,
        "centro" => 1,
        "direita" => 2,
        _ => return Err(format!("Lugar de número de página desconhecido: \"{onde}\".")),
    };
    Ok((topo, lado))
}

fn slot(f: &mut Faixas, lado: u8) -> &mut String {
    match lado {
        0 => &mut f.esquerda,
        1 => &mut f.centro,
        _ => &mut f.direita,
    }
}

fn tem_numero(t: &str) -> bool {
    t.contains("{pagina}") || t.contains("{pagina_romano}")
}

impl Sessao {
    /// Muda o que vale para a página toda e repinta (não é bloco: fica fora do desfazer, como o papel).
    pub fn mudar_extras(&mut self, f: impl FnOnce(&mut Extras)) -> Mudanca {
        let antes = self.doc.extras.clone();
        f(&mut self.doc.extras);
        if antes == self.doc.extras {
            return self.sem_mudanca();
        }
        self.fim_da_edicao_sem_blocos()
    }

    /// O cabeçalho: texto da esquerda, do centro e da direita (com os campos `{pagina}`, `{total}`…).
    pub fn cabecalho_definir(&mut self, esquerda: &str, centro: &str, direita: &str) -> Mudanca {
        // O texto muda; o fio e a letra ficam.
        let novo = Faixas { esquerda: esquerda.to_string(), centro: centro.to_string(), direita: direita.to_string(), ..self.doc.extras.cabecalho.clone() };
        self.mudar_extras(|x| x.cabecalho = novo)
    }

    /// O rodapé, como o cabeçalho.
    pub fn rodape_definir(&mut self, esquerda: &str, centro: &str, direita: &str) -> Mudanca {
        // O texto muda; o fio e a letra ficam.
        let novo = Faixas { esquerda: esquerda.to_string(), centro: centro.to_string(), direita: direita.to_string(), ..self.doc.extras.rodape.clone() };
        self.mudar_extras(|x| x.rodape = novo)
    }

    /// A primeira página (a capa) sem cabeçalho e rodapé.
    pub fn primeira_diferente_definir(&mut self, sim: bool) -> Mudanca {
        self.mudar_extras(|x| x.primeira_diferente = sim)
    }

    /// Põe o número de página (`modelo`, por exemplo "Página {pagina} de {total}") no lugar pedido,
    /// tirando o número que estivesse em outro lugar.
    pub fn numero_de_pagina(&mut self, modelo: &str, onde: &str) -> Result<Mudanca, String> {
        let (topo, lado) = lugar(onde)?;
        if !tem_numero(modelo) {
            return Err("O modelo do número de página precisa ter {pagina} ou {pagina_romano}.".to_string());
        }
        let modelo = modelo.to_string();
        Ok(self.mudar_extras(move |x| {
            tirar_numeros(x);
            *slot(if topo { &mut x.cabecalho } else { &mut x.rodape }, lado) = modelo;
        }))
    }

    /// Tira o número de página do cabeçalho e do rodapé.
    pub fn numero_remover(&mut self) -> Mudanca {
        self.mudar_extras(tirar_numeros)
    }

    /// Cor do papel (`#rrggbb`; vazio = branco).
    pub fn cor_pagina_definir(&mut self, cor: &str) -> Result<Mudanca, String> {
        let c = ler_cor(cor)?;
        Ok(self.mudar_extras(|x| x.cor_pagina = c))
    }

    /// Marca d'água em diagonal (texto vazio tira); `cor` vazia = cinza claro.
    pub fn marca_dagua_definir(&mut self, texto: &str, cor: &str) -> Result<Mudanca, String> {
        let c = ler_cor(cor)?;
        let t = texto.trim().to_string();
        Ok(self.mudar_extras(|x| {
            x.marca = t;
            x.marca_cor = if c == 0 { 0 } else { (c & 0xffff_ff00) | 0x66 };
        }))
    }

    /// Arquivo → Propriedades: assunto, palavras-chave, descrição e empresa (vão para o `.docx`).
    pub fn propriedades_definir(&mut self, assunto: &str, palavras_chave: &str, descricao: &str, empresa: &str) -> Mudanca {
        let v = [assunto, palavras_chave, descricao, empresa].map(|t| t.trim().to_string());
        self.mudar_extras(|x| {
            let [a, p, d, e] = v;
            let m = &mut x.documento;
            (m.assunto, m.palavras_chave, m.descricao, m.empresa) = (a, p, d, e);
        })
    }

    /// Borda da página: cor (vazia tira), espessura e distância da borda do papel, em pontos.
    pub fn borda_pagina_definir(&mut self, cor: &str, espessura: f32, distancia: f32) -> Result<Mudanca, String> {
        let c = ler_cor(cor)?;
        let p = self.doc.pagina;
        if !(0.0..=12.0).contains(&espessura) || distancia < 0.0 || distancia * 2.0 >= p.largura.min(p.altura) {
            return Err("Borda da página inválida: espessura de 0 a 12 pt e distância menor que meia página.".to_string());
        }
        Ok(self.mudar_extras(|x| {
            x.borda_cor = c;
            x.borda_espessura = espessura;
            x.borda_distancia = distancia;
        }))
    }

    /// Mostra ou esconde as marcas de formatação (¶ no fim do parágrafo, · nos espaços).
    /// É só de tela: não suja o documento.
    pub fn marcas_definir(&mut self, sim: bool) -> Mudanca {
        if self.marcas == sim {
            return self.sem_mudanca();
        }
        self.marcas = sim;
        self.nova_versao_de_tela()
    }

    /// Linhas de grade sobre a área do texto (só na tela, não suja o documento).
    pub fn grade_definir(&mut self, sim: bool) -> Mudanca {
        if self.grade == sim {
            return self.sem_mudanca();
        }
        self.grade = sim;
        self.nova_versao_de_tela()
    }

    /// Layout → Alinhamento vertical: `topo`, `centro` ou `base`.
    pub fn vertical_definir(&mut self, nome: &str) -> Result<Mudanca, String> {
        let v = match nome {
            "topo" => Vertical::Topo,
            "centro" => Vertical::Centro,
            "base" => Vertical::Base,
            outro => return Err(format!("Não conheço o alinhamento vertical \"{outro}\".")),
        };
        Ok(self.mudar_extras(|x| x.vertical = v))
    }

    /// Layout → Números de linha: `nenhuma`, `continua` ou `por_pagina`.
    pub fn numerar_linhas_definir(&mut self, nome: &str) -> Result<Mudanca, String> {
        let n = match nome {
            "nenhuma" => NumeracaoDeLinhas::Nenhuma,
            "continua" => NumeracaoDeLinhas::Continua,
            "por_pagina" => NumeracaoDeLinhas::PorPagina,
            outro => return Err(format!("Não conheço a numeração de linhas \"{outro}\".")),
        };
        Ok(self.mudar_extras(|x| x.numerar_linhas = n))
    }

    /// As marcas de formatação estão à vista?
    pub fn marcas_visiveis(&self) -> bool {
        self.marcas
    }
}

fn tirar_numeros(x: &mut Extras) {
    for f in [&mut x.cabecalho, &mut x.rodape] {
        for lado in 0..3 {
            let s = slot(f, lado);
            if tem_numero(s) {
                s.clear();
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::quadro::OP_GIRADO;

    fn tem_texto(q: &[u8], t: &str) -> bool {
        q.windows(t.len()).any(|w| w == t.as_bytes())
    }

    #[test]
    fn rodape_padrao_e_numero_de_pagina_com_total() {
        let mut s = Sessao::default();
        assert!(tem_texto(&s.quadro(0, 0).unwrap(), "1"));
        s.numero_de_pagina("Página {pagina} de {total}", "topo_direita").unwrap();
        let q = s.quadro(0, 0).unwrap();
        assert!(tem_texto(&q, "Página 1 de 1"));
        assert!(s.documento().extras.rodape.vazia(), "o número saiu do rodapé");
        assert!(s.numero_de_pagina("sem campo", "base_centro").is_err());
        assert!(s.numero_de_pagina("{pagina}", "meio").is_err());
        s.numero_remover();
        assert!(s.documento().extras.cabecalho.vazia());
    }

    #[test]
    fn cabecalho_com_titulo_e_primeira_diferente() {
        let mut s = Sessao::default();
        s.meta_definir("Relatório", "Ana");
        s.cabecalho_definir("{titulo}", "", "{autor}");
        assert!(tem_texto(&s.quadro(0, 0).unwrap(), "Relatório"));
        s.primeira_diferente_definir(true);
        let q = s.quadro(0, 0).unwrap();
        assert!(!tem_texto(&q, "Relatório"), "a capa fica sem cabeçalho");
    }

    #[test]
    fn plano_de_fundo_marca_dagua_e_borda() {
        let mut s = Sessao::default();
        s.cor_pagina_definir("#fff8e1").unwrap();
        s.marca_dagua_definir("RASCUNHO", "").unwrap();
        s.borda_pagina_definir("#1a73e8", 1.5, 24.0).unwrap();
        let q = s.quadro(0, 0).unwrap();
        assert!(q.contains(&OP_GIRADO) && tem_texto(&q, "RASCUNHO"));
        assert!(s.borda_pagina_definir("#000000", 50.0, 0.0).is_err());
        assert!(s.cor_pagina_definir("amarelo").is_err());
        assert!(!s.documento().extras.marca.is_empty());
    }

    #[test]
    fn marcas_de_formatacao_nao_sujam_o_documento() {
        let mut s = Sessao::default();
        s.inserir_texto("a b");
        s.marcar_gravado();
        s.marcas_definir(true);
        assert!(!s.estado().sujo);
        let q = s.quadro(0, 0).unwrap();
        assert!(tem_texto(&q, "¶") && tem_texto(&q, "·"));
    }

    #[test]
    fn alinhamento_vertical_desce_o_texto_e_o_cursor_junto() {
        let mut s = Sessao::default();
        s.inserir_texto("capa");
        let antes = s.paginas[0].fatias[0].y;
        s.vertical_definir("centro").unwrap();
        let meio = s.paginas[0].fatias[0].y;
        assert!(meio > antes + 300.0, "o texto foi para o meio da página");
        let g = s.geometria(&s.cursor.foco).unwrap();
        assert!(g.y > s.documento().pagina.margem_sup + 300.0, "o cursor desce com o texto");
        s.vertical_definir("base").unwrap();
        assert!(s.paginas[0].fatias[0].y > meio);
        assert!(s.vertical_definir("lado").is_err());
    }

    #[test]
    fn numeros_de_linha_continuos_e_por_pagina() {
        let mut s = Sessao::default();
        for _ in 0..80 {
            s.inserir_texto("linha");
            s.tecla("Enter", false, false, false);
        }
        assert!(s.total_de_paginas() >= 2);
        s.numerar_linhas_definir("continua").unwrap();
        let ja = s.linhas_antes(1);
        assert!(ja > 30);
        assert!(tem_texto(&s.quadro(1, 0).unwrap(), &(ja + 1).to_string()));
        s.numerar_linhas_definir("por_pagina").unwrap();
        let q = s.quadro(1, 0).unwrap();
        assert!(!tem_texto(&q, &(ja + 1).to_string()), "recomeça na página");
    }

    #[test]
    fn grade_e_so_de_tela() {
        let mut s = Sessao::default();
        s.marcar_gravado();
        let sem = s.quadro(0, 0).unwrap().len();
        s.grade_definir(true);
        assert!(!s.estado().sujo);
        assert!(s.quadro(0, 0).unwrap().len() > sem + 500, "as linhas da grade entraram no quadro");
    }
}

