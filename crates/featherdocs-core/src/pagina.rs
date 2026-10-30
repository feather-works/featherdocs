//! O que vale para a página inteira e não é bloco: cabeçalho, rodapé, número de página,
//! cor do papel, marca d'água, borda da página e as marcas de formatação (¶ ·) à vista.
//! O desenho disso tudo está em `quadro.rs`.

use crate::fonte::ler_cor;
use crate::modelo::{Conteudo, Extras, Faixas, NumeracaoDeLinhas, Vertical};
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

    /// Tira a imagem `id` do texto e a prende à página, onde ela está desenhada agora: o texto
    /// em volta muda e ela não "pula" mais. Um Ctrl+Z. A âncora (a página em que ela aparece) é o
    /// bloco seguinte, ou o anterior.
    pub fn imagem_fixar_na_pagina(&mut self, id: u64) -> Result<Mudanca, String> {
        use crate::layout::Deco;
        let i = self.idx(id).ok_or("A imagem não existe mais.")?;
        let origem = match &self.doc.blocos[i].conteudo {
            Conteudo::Imagem { origem, legenda, .. } if legenda.is_empty() => origem.clone(),
            Conteudo::Imagem { .. } => return Err("Imagem com legenda fica no texto, junto da legenda.".into()),
            _ => return Err("Clique numa imagem para fixá-la na página.".into()),
        };
        let (pg, fx, fy) = self.fatias_do_bloco(i).first().map(|(p, f)| (*p, f.x, f.y)).ok_or("A imagem ainda não está numa página.")?;
        let (x, y, largura, altura) = self.layouts[i].linhas.first().and_then(|l| l.deco.iter().find_map(|d| match d {
            Deco::Imagem { x, y, w, h, .. } => Some((*x, *y, *w, *h)),
            _ => None,
        })).ok_or("A imagem não foi desenhada.")?;
        let cfg = self.cfg_da_pagina(pg);
        let mut dentro = Vec::new();
        let vizinho = if i + 1 < self.doc.blocos.len() { Some(i + 1) } else { i.checked_sub(1) };
        let (ancora, pos) = match vizinho {
            Some(k) => (self.doc.blocos[k].id, self.inicio_do_bloco(k)),
            // Era o único bloco: fica um parágrafo vazio no lugar, que a segura.
            None => {
                let vazio = self.doc.novo_bloco(Conteudo::Paragrafo { texto: Vec::new() });
                let id = vazio.id;
                dentro.push(vazio);
                (id, crate::cursor::Pos { bloco: id, celula: None, desl: 0 })
            }
        };
        let antes = self.doc.extras.clone();
        self.doc.extras.decoracoes.push(crate::modelo::Decoracao {
            origem,
            x: cfg.margem_esq + fx + x,
            y: cfg.margem_sup + fy + y,
            largura,
            altura,
            atras: false,
            bloco: Some(ancora),
        });
        Ok(self.transacao_com_extras(crate::sessao::Splice { pos: i, quantos: 1, dentro }, crate::cursor::Cursor::em(pos), antes))
    }

    /// A figura presa à página sob o ponto (`x`, `y`) da página `pagina` (a da frente primeiro), se
    /// houver uma ancorada a um bloco (as de toda página são do timbre e ficam onde estão).
    pub fn figura_em(&self, pagina: usize, x: f32, y: f32) -> Option<usize> {
        let ds = &self.doc.extras.decoracoes;
        let dentro = |k: &usize| {
            let d = &ds[*k];
            d.bloco.is_some() && self.decoracao_na_pagina(d, pagina) && (d.x..=d.x + d.largura).contains(&x) && (d.y..=d.y + d.altura).contains(&y)
        };
        let ordem = (0..ds.len()).rev();
        ordem.clone().filter(|k| !ds[*k].atras).find(dentro).or_else(|| ordem.filter(|k| ds[*k].atras).find(dentro))
    }

    /// Volta a figura `k` presa à página para o texto, como imagem antes do bloco onde está
    /// ancorada (o contrário de [`Sessao::imagem_fixar_na_pagina`]). Um Ctrl+Z.
    pub fn figura_colocar_no_texto(&mut self, k: usize) -> Result<Mudanca, String> {
        let d = self.doc.extras.decoracoes.get(k).cloned().ok_or("A figura não existe mais.")?;
        let i = d.bloco.and_then(|id| self.idx(id)).ok_or("Esta figura é de toda página (o timbre).")?;
        let antes = self.doc.extras.clone();
        self.doc.extras.decoracoes.remove(k);
        let imagem = self.doc.novo_bloco(Conteudo::Imagem { origem: d.origem, legenda: Vec::new(), largura: d.largura, altura: d.altura });
        let cursor = crate::cursor::Cursor::em(crate::cursor::Pos { bloco: imagem.id, celula: None, desl: 0 });
        Ok(self.transacao_com_extras(crate::sessao::Splice { pos: i, quantos: 0, dentro: vec![imagem] }, cursor, antes))
    }

    /// Arrasta a imagem solta `id` para o ponto (`x`, `y`) da página `pagina`: ela passa a ser
    /// contornada pelo parágrafo sob o ponto, à esquerda ou à direita conforme o lado da coluna. Um Ctrl+Z.
    pub fn imagem_solta_mover(&mut self, id: u64, pagina: usize, x: f32, y: f32) -> Result<Mudanca, String> {
        use crate::modelo::PosicaoFlutuante;
        let i = self.idx(id).ok_or("A imagem não existe mais.")?;
        let mut imagem = self.doc.blocos[i].clone();
        let f = imagem.imagem.flutuante.as_mut().ok_or("A imagem não está solta do texto.")?;
        let p = self.pos_na_pagina(pagina, x, y).ok_or("Solte a imagem sobre o texto.")?;
        let alvo = self.idx(p.bloco).ok_or("Solte a imagem sobre o texto.")?;
        if !matches!(self.doc.blocos[alvo].conteudo, Conteudo::Paragrafo { .. }) {
            return Err("Solte a imagem sobre um parágrafo.".into());
        }
        let cfg = self.cfg_da_pagina(pagina);
        let coluna = self.fatias_do_bloco(alvo).iter().find(|(pg, _)| *pg == pagina).map_or(0.0, |(_, f)| f.x);
        let a_direita = x - cfg.margem_esq - coluna > self.doc.colunas_em(&cfg).1 / 2.0;
        f.horizontal = PosicaoFlutuante { relativo: "column".into(), alinhar: if a_direita { "right" } else { "left" }.into(), deslocamento: 0.0 };
        f.vertical = PosicaoFlutuante { relativo: "paragraph".into(), alinhar: String::new(), deslocamento: 0.0 };
        if imagem == self.doc.blocos[i] && (alvo == i || alvo == i + 1) {
            return Ok(self.sem_mudanca());
        }
        let blocos = &self.doc.blocos;
        let s = if alvo > i + 1 {
            let mut dentro = blocos[i + 1..alvo].to_vec();
            dentro.push(imagem);
            crate::sessao::Splice { pos: i, quantos: alvo - i, dentro }
        } else if alvo < i {
            let mut dentro = vec![imagem];
            dentro.extend_from_slice(&blocos[alvo..i]);
            crate::sessao::Splice { pos: alvo, quantos: i - alvo + 1, dentro }
        } else {
            crate::sessao::Splice { pos: i, quantos: 1, dentro: vec![imagem] }
        };
        let cursor = self.cursor.clone();
        Ok(self.transacao(s, cursor, None))
    }

    /// Entra (ou sai, com `None`) na edição do cabeçalho (`rodape = false`) ou do rodapé da página,
    /// na própria página: o resto esmaece e a faixa ganha a linha tracejada, como no Word.
    pub fn faixa_editar(&mut self, onde: Option<(usize, bool)>) -> Mudanca {
        if self.faixa_em_edicao == onde {
            return self.sem_mudanca();
        }
        self.faixa_em_edicao = onde;
        self.nova_versao_de_tela()
    }

    pub fn faixa_em_edicao(&self) -> Option<(usize, bool)> {
        self.faixa_em_edicao
    }

    /// A página `pagina` tem o ponto `y` na margem de cima (`Some(false)`) ou na de baixo (`Some(true)`).
    pub fn faixa_no_ponto(&self, pagina: usize, y: f32) -> Option<bool> {
        let cfg = self.cfg_da_pagina(pagina);
        if y < cfg.margem_sup {
            Some(false)
        } else if y > cfg.altura - cfg.margem_inf {
            Some(true)
        } else {
            None
        }
    }

    /// Os três textos (esquerda, centro, direita) da faixa que a página mostra, com os campos por resolver.
    pub fn faixa_textos(&self, pagina: usize, rodape: bool) -> [String; 3] {
        let f = self.faixa_da_pagina(pagina, rodape);
        [f.esquerda.clone(), f.centro.clone(), f.direita.clone()]
    }

    /// A faixa só de texto (sem tabela nem vários parágrafos do Word) pode ser editada na página.
    pub fn faixa_editavel(&self, pagina: usize, rodape: bool) -> bool {
        !self.faixa_da_pagina(pagina, rodape).blocos_valem()
    }

    /// Troca os textos da faixa que a página mostra (a da primeira página, a das pares ou a de sempre,
    /// da seção dela). O texto não mexe na paginação: só repinta.
    pub fn faixa_textos_definir(&mut self, pagina: usize, rodape: bool, t: [String; 3]) -> Mudanca {
        let (k, tipo) = self.faixas_da_pagina(pagina);
        let trocar = |f: &mut Faixas| (f.esquerda, f.centro, f.direita) = (t[0].clone(), t[1].clone(), t[2].clone());
        if k + 1 >= self.total_de_secoes() {
            return self.mudar_extras(|x| trocar(match (tipo, rodape) {
                (1, false) => &mut x.cabecalho_primeira,
                (1, true) => &mut x.rodape_primeira,
                (2, false) => &mut x.documento.cabecalho_par,
                (2, true) => &mut x.documento.rodape_par,
                (_, false) => &mut x.cabecalho,
                (_, true) => &mut x.rodape,
            }));
        }
        self.mudar_secao(k, |s| trocar(match (tipo, rodape) {
            (1, false) => &mut s.cabecalho_primeira,
            (1, true) => &mut s.rodape_primeira,
            (2, false) => &mut s.cabecalho_par,
            (2, true) => &mut s.rodape_par,
            (_, false) => &mut s.cabecalho,
            (_, true) => &mut s.rodape,
        }))
    }

    /// Onde fica cada um dos três textos da faixa: (x de começo, largura), na página.
    fn faixa_posicoes(&self, pagina: usize, rodape: bool) -> [(f32, f32); 3] {
        let cfg = self.cfg_da_pagina(pagina);
        let f = self.faixa_da_pagina(pagina, rodape);
        let tam = if f.tamanho > 0.0 { f.tamanho } else { 9.0 };
        let w = |t: &str| crate::medida::largura(t, f.estilo, tam);
        let (a, b, c) = (w(&f.esquerda), w(&f.centro), w(&f.direita));
        [(cfg.margem_esq, a), ((cfg.largura - b) / 2.0, b), (cfg.largura - cfg.margem_dir - c, c)]
    }

    /// O cursor na faixa em edição: o texto `campo` (0 esquerda, 1 centro, 2 direita) e o caractere `desl`
    /// → (x, topo, altura) na página.
    pub fn faixa_cursor(&self, pagina: usize, rodape: bool, campo: usize, desl: usize) -> (f32, f32, f32) {
        let f = self.faixa_da_pagina(pagina, rodape);
        let tam = if f.tamanho > 0.0 { f.tamanho } else { 9.0 };
        let texto = [&f.esquerda, &f.centro, &f.direita][campo.min(2)];
        let antes: String = texto.chars().take(desl).collect();
        let x = self.faixa_posicoes(pagina, rodape)[campo.min(2)].0 + crate::medida::largura(&antes, f.estilo, tam);
        let base = self.linha_da_faixa(pagina, rodape);
        (x, base - tam, tam * 1.25)
    }

    /// O clique na faixa em edição: em que texto e em que caractere. Fora dos três, o terço da largura decide.
    pub fn faixa_campo_no_ponto(&self, pagina: usize, rodape: bool, x: f32) -> (usize, usize) {
        let f = self.faixa_da_pagina(pagina, rodape);
        let tam = if f.tamanho > 0.0 { f.tamanho } else { 9.0 };
        let pos = self.faixa_posicoes(pagina, rodape);
        let largura_pagina = self.cfg_da_pagina(pagina).largura;
        let campo = (0..3).find(|&k| pos[k].1 > 0.0 && (pos[k].0 - 4.0..=pos[k].0 + pos[k].1 + 4.0).contains(&x))
            .unwrap_or(((x / largura_pagina * 3.0) as usize).min(2));
        let texto = [&f.esquerda, &f.centro, &f.direita][campo];
        // O caractere cuja metade fica depois do ponto.
        let mut desl = 0;
        for (n, _) in texto.chars().enumerate() {
            let ate: String = texto.chars().take(n + 1).collect();
            let meio = pos[campo].0 + crate::medida::largura(&ate, f.estilo, tam) - crate::medida::largura(&ate.chars().last().unwrap().to_string(), f.estilo, tam) / 2.0;
            if x < meio {
                break;
            }
            desl = n + 1;
        }
        (campo, desl)
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

    /// Layout → Colunas: o texto do documento em `n` colunas (1 a 4), com o espaço padrão entre elas.
    pub fn colunas_definir(&mut self, n: u8) -> Mudanca {
        self.mudar_extras(|x| x.documento.colunas = if n <= 1 { 0 } else { n.min(4) })
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
    fn imagem_fixada_na_pagina_sai_do_texto_no_mesmo_lugar_e_desfaz() {
        let mut d = crate::modelo::Documento::vazio();
        d.acrescentar(Conteudo::Paragrafo { texto: vec![crate::modelo::Trecho::simples("antes")] });
        d.acrescentar(Conteudo::Imagem { origem: "data:image/png;base64,AA==".into(), legenda: Vec::new(), largura: 100.0, altura: 50.0 });
        d.acrescentar(Conteudo::Paragrafo { texto: vec![crate::modelo::Trecho::simples("depois")] });
        let mut s = Sessao::com_documento(d);
        let antes = s.documento().clone();
        let id = s.documento().blocos[1].id;
        s.imagem_fixar_na_pagina(id).unwrap();
        assert_eq!(s.documento().blocos.len(), 2);
        let f = &s.documento().extras.decoracoes[0];
        assert_eq!((f.largura, f.altura, f.bloco), (100.0, 50.0, Some(s.documento().blocos[1].id)));
        assert!(f.x >= s.documento().pagina.margem_esq && f.y > s.documento().pagina.margem_sup);
        s.desfazer();
        assert_eq!(s.documento(), &antes);
        s.refazer();
        assert_eq!(s.documento().extras.decoracoes.len(), 1);
        // Clicar nela a acha; voltar ao texto põe a imagem antes do bloco da âncora.
        let f = s.documento().extras.decoracoes[0].clone();
        let k = s.figura_em(0, f.x + 5.0, f.y + 5.0).expect("a figura está sob o clique");
        assert!(s.figura_em(0, f.x - 5.0, f.y - 5.0).is_none());
        s.figura_colocar_no_texto(k).unwrap();
        assert!(s.documento().extras.decoracoes.is_empty());
        assert!(matches!(s.documento().blocos[1].conteudo, Conteudo::Imagem { largura: 100.0, .. }));
        assert!(s.imagem_fixar_na_pagina(s.documento().blocos[0].id).is_err(), "parágrafo não é imagem");
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

    #[test]
    fn cabecalho_editado_na_propria_pagina() {
        let mut s = Sessao::com_documento(crate::modelo::Documento::vazio());
        s.inserir_texto("corpo");
        let cfg = s.cfg_da_pagina(0);
        assert_eq!(s.faixa_no_ponto(0, 10.0), Some(false));
        assert_eq!(s.faixa_no_ponto(0, cfg.altura - 10.0), Some(true));
        assert_eq!(s.faixa_no_ponto(0, cfg.altura / 2.0), None);
        let (v, sujo) = (s.versao(), s.estado().sujo);
        s.faixa_editar(Some((0, false)));
        assert!(s.versao() > v && s.estado().sujo == sujo, "só a tela muda");
        s.faixa_textos_definir(0, false, ["Empresa".into(), String::new(), "p. {pagina}".into()]);
        assert_eq!(s.documento().extras.cabecalho.esquerda, "Empresa");
        assert_eq!(s.faixa_textos(0, false)[2], "p. {pagina}");
        let q = String::from_utf8_lossy(&s.quadro(0, 0).unwrap()).to_string();
        assert!(q.contains("p. {pagina}") && q.contains("Cabeçalho"), "em edição, o campo aparece como é");
        // O cursor e o clique se correspondem.
        let (x, ..) = s.faixa_cursor(0, false, 0, 3);
        assert_eq!(s.faixa_campo_no_ponto(0, false, x + 0.5), (0, 3));
        let (x, ..) = s.faixa_cursor(0, false, 2, 0);
        assert_eq!(s.faixa_campo_no_ponto(0, false, x + 0.5), (2, 0));
        s.faixa_editar(None);
        let q = String::from_utf8_lossy(&s.quadro(0, 0).unwrap()).to_string();
        assert!(q.contains("p. 1") && !q.contains("Cabeçalho"));
    }
}
