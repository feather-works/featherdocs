//! Revisão: controle de alterações (o que se digita ou apaga vira sugestão, até aceitar ou
//! rejeitar), comentários ligados a um trecho e os modos que travam a edição (somente leitura e
//! documento protegido).
//!
//! O controle de alterações não mexe em cada comando de edição: a edição acontece como sempre e,
//! antes de entrar no documento, `como_sugestao` compara o texto de antes com o de depois em cada
//! bloco — o que entrou fica marcado como inserção, o que saiu volta marcado como remoção. Só as
//! edições dentro dos blocos são rastreadas; partir ou juntar parágrafos acontece direto.

use serde::Serialize;

use crate::alvo::{texto_alvo, Cel};
use crate::cursor::{Cursor, Pos};
use crate::digitacao::Trabalho;
use crate::formato::faixas;
use crate::inline::texto_puro;
use crate::modelo::{Comentario, Conteudo, Marca, Resposta, Revisao, TipoDeRevisao, Trecho};
use crate::sessao::{Mudanca, Sessao, Splice};
use crate::texto::{fatia, juntar, normalizar, plano, tamanho};

/// O texto `novo` como sugestão sobre o `antigo`: o miolo que mudou volta com o que saiu (riscado)
/// antes do que entrou (sublinhado). Apagar uma inserção do mesmo autor apaga de verdade. Devolve
/// o texto, o começo do miolo e quantos caracteres riscados ficaram (para ajustar o cursor).
pub(crate) fn sugestao(antigo: &[Trecho], novo: &[Trecho], rev: &Revisao) -> (Vec<Trecho>, usize, usize) {
    let (a, n): (Vec<char>, Vec<char>) = (plano(antigo).chars().collect(), plano(novo).chars().collect());
    let p = a.iter().zip(&n).take_while(|(x, y)| x == y).count();
    let max_s = a.len().min(n.len()) - p;
    let s = a.iter().rev().zip(n.iter().rev()).take(max_s).take_while(|(x, y)| x == y).count();
    let (fim_a, fim_n) = (a.len() - s, n.len() - s);
    if p == fim_a && p == fim_n {
        return (novo.to_vec(), p, 0);
    }
    let saiu: Vec<Trecho> = fatia(antigo, p, fim_a)
        .into_iter()
        .filter(|t| !matches!(t.rev.as_deref(), Some(r) if r.tipo == TipoDeRevisao::Insercao && r.autor == rev.autor))
        .map(|mut t| {
            if t.rev.is_none() {
                t.rev = Some(Box::new(Revisao { tipo: TipoDeRevisao::Remocao, ..rev.clone() }));
            }
            t
        })
        .collect();
    let riscados = tamanho(&saiu);
    let entrou: Vec<Trecho> = fatia(novo, p, fim_n)
        .into_iter()
        .map(|mut t| {
            if t.rev.is_none() {
                t.rev = Some(Box::new(Revisao { tipo: TipoDeRevisao::Insercao, ..rev.clone() }));
            }
            t
        })
        .collect();
    let mut texto = juntar(juntar(juntar(fatia(novo, 0, p), saiu), entrou), fatia(novo, fim_n, usize::MAX));
    normalizar(&mut texto);
    (texto, p, riscados)
}

/// A posição do texto (na ordem de `Conteudo::textos`) de um alvo: 0 fora de tabela, a célula na tabela.
fn indice_do_alvo(c: &Conteudo, cel: Cel) -> usize {
    match (c, cel) {
        (Conteudo::Tabela { linhas, .. }, Some((l, k))) => linhas.iter().take(l as usize).map(Vec::len).sum::<usize>() + k as usize,
        _ => 0,
    }
}

/// Aceita (ou rejeita) as sugestões do texto `[a, b)`: aceitar uma inserção a deixa, aceitar uma
/// remoção apaga o texto; rejeitar faz o contrário.
fn resolver(t: &mut Vec<Trecho>, a: usize, b: usize, aceitar: bool) -> bool {
    let meio = fatia(t, a, b);
    if !meio.iter().any(|x| x.rev.is_some()) {
        return false;
    }
    let resolvido: Vec<Trecho> = meio
        .into_iter()
        .filter_map(|mut x| {
            let fica = match x.rev.as_deref().map(|r| r.tipo) {
                Some(TipoDeRevisao::Insercao) => aceitar,
                Some(TipoDeRevisao::Remocao) => !aceitar,
                Some(TipoDeRevisao::Formatacao) => {
                    // Rejeitar volta à formatação de antes.
                    if let Some((estilo, fmt)) = x.rev.as_ref().and_then(|r| r.antes.as_deref()).filter(|_| !aceitar) {
                        (x.estilo, x.fmt) = (*estilo, *fmt);
                    }
                    true
                }
                None => true,
            };
            x.rev = None;
            fica.then_some(x)
        })
        .collect();
    *t = juntar(juntar(fatia(t, 0, a), resolvido), fatia(t, b, usize::MAX));
    true
}

/// As sugestões do bloco: aceitar as deixa; rejeitar volta o parágrafo de antes, tira as linhas
/// inseridas e devolve as apagadas.
// shortcut: rejeitar a marca de parágrafo inserida não junta o parágrafo com o seguinte; só tira a marca.
fn resolver_bloco(b: &mut crate::modelo::Bloco, aceitar: bool) -> bool {
    let Some(r) = b.revisao.take() else { return false };
    if !aceitar {
        if let Some((_, antes)) = r.paragrafo {
            b.atrib = antes;
        }
    }
    if let crate::modelo::Conteudo::Tabela { linhas, .. } = &mut b.conteudo {
        // Do fim para o começo, para os índices valerem.
        for (i, rev) in r.linhas.iter().rev() {
            let sai = if rev.tipo == TipoDeRevisao::Insercao { !aceitar } else { aceitar };
            if sai && *i < linhas.len() && linhas.len() > 1 {
                linhas.remove(*i);
            }
        }
    }
    true
}

/// Um comentário com o que a lista lateral mostra.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InfoComentario {
    pub id: u64,
    pub autor: String,
    pub quando: String,
    pub texto: String,
    pub resolvido: bool,
    pub respostas: Vec<Resposta>,
    /// O texto comentado (cortado se for comprido).
    pub trecho: String,
    pub bloco: u64,
    /// A página (1, 2…).
    pub pagina: usize,
}

/// Uma alteração sugerida, para a lista lateral.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InfoRevisao {
    pub tipo: TipoDeRevisao,
    pub autor: String,
    pub quando: String,
    pub texto: String,
    pub bloco: u64,
}

impl Sessao {
    /// As edições viram sugestões agora?
    pub(crate) fn rastreando(&self) -> bool {
        self.doc.extras.controlar_alteracoes && !self.rastreio_pausado
    }

    /// Nada pode mudar o documento (somente leitura ou protegido); comentar ainda pode.
    pub fn bloqueado(&self) -> bool {
        self.somente_leitura || self.doc.extras.protegido
    }

    fn revisao(&self, tipo: TipoDeRevisao) -> Revisao {
        Revisao { tipo, autor: if self.autor.is_empty() { "Autor".to_string() } else { self.autor.clone() }, quando: self.hoje.clone(), movido: false, antes: None }
    }

    /// Transforma uma edição em sugestão (só quando os blocos são os mesmos antes e depois: edição
    /// dentro do texto). O cursor anda junto com os riscados que voltaram.
    pub(crate) fn como_sugestao(&self, mut s: Splice, mut cursor: Cursor) -> (Splice, Cursor) {
        let fim = (s.pos + s.quantos).min(self.doc.blocos.len());
        let antigos = &self.doc.blocos[s.pos..fim];
        let mesmos = antigos.len() == s.dentro.len() && antigos.iter().zip(&s.dentro).all(|(a, n)| a.id == n.id);
        if !mesmos {
            return (s, cursor);
        }
        let rev = self.revisao(TipoDeRevisao::Insercao);
        for (velho, novo) in antigos.iter().zip(s.dentro.iter_mut()) {
            let textos_velhos = velho.conteudo.textos();
            let alvo = (cursor.foco.bloco == novo.id).then(|| indice_do_alvo(&novo.conteudo, cursor.foco.celula));
            let mut textos_novos = novo.conteudo.textos_mut();
            if textos_velhos.len() != textos_novos.len() {
                continue;
            }
            for (k, (v, n)) in textos_velhos.into_iter().zip(textos_novos.iter_mut()).enumerate() {
                let (texto, p, riscados) = sugestao(v, n, &rev);
                **n = texto;
                if alvo == Some(k) {
                    for q in [&mut cursor.foco, &mut cursor.ancora] {
                        // O Delete (o cursor não andou) também passa por cima do que riscou.
                        if q.bloco == novo.id && (q.desl > p || q.desl == p && self.cursor.foco.desl == p) {
                            q.desl += riscados;
                        }
                    }
                }
            }
        }
        (s, cursor)
    }

    /// Com o controle de alterações, apagar uma seleção que passa de um bloco a outro não apaga:
    /// risca. Devolve a mudança (e o cursor fica no fim da seleção), ou `None` se não é o caso.
    pub(crate) fn riscar_selecao_entre_blocos(&mut self) -> Option<Mudanca> {
        if !self.rastreando() || self.cursor.vazio() {
            return None;
        }
        let (mut t, ini, fim) = self.trabalho_da_selecao();
        if ini.bloco == fim.bloco {
            return None;
        }
        let rev = self.revisao(TipoDeRevisao::Remocao);
        for f in faixas(&t, ini, fim) {
            crate::alvo::com_texto_ref(&mut t.blocos[f.bloco].conteudo, f.celula, |x| {
                let meio: Vec<Trecho> = fatia(x, f.a, f.b)
                    .into_iter()
                    .filter(|tr| !matches!(tr.rev.as_deref(), Some(r) if r.tipo == TipoDeRevisao::Insercao && r.autor == rev.autor))
                    .map(|mut tr| {
                        if tr.rev.is_none() {
                            tr.rev = Some(Box::new(rev.clone()));
                        }
                        tr
                    })
                    .collect();
                *x = juntar(juntar(fatia(x, 0, f.a), meio), fatia(x, f.b, usize::MAX));
            });
        }
        let fim_texto = t.blocos.iter().find(|b| b.id == fim.bloco).and_then(|b| texto_alvo(&b.conteudo, fim.celula)).map_or(0, |x| tamanho(&x));
        let cursor = Cursor::em(Pos { desl: fim.desl.min(fim_texto), ..fim });
        Some(self.sem_rastrear(|s| s.fechar_trabalho(t, cursor, None)))
    }

    /// Roda `f` com o controle de alterações desligado (aceitar, rejeitar, comparar).
    pub(crate) fn sem_rastrear<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let antes = self.rastreio_pausado;
        self.rastreio_pausado = true;
        let r = f(self);
        self.rastreio_pausado = antes;
        r
    }

    /// Revisão → Modo: `editando`, `sugerindo` (controle de alterações) ou `leitura`.
    pub fn modo_edicao_definir(&mut self, modo: &str) -> Result<Mudanca, String> {
        let (controlar, leitura) = match modo {
            "editando" => (false, false),
            "sugerindo" => (true, false),
            "leitura" => (self.doc.extras.controlar_alteracoes, true),
            _ => return Err(format!("Não conheço o modo \"{modo}\".")),
        };
        self.somente_leitura = leitura;
        if self.doc.extras.controlar_alteracoes == controlar {
            return Ok(self.nova_versao_de_tela());
        }
        self.doc.extras.controlar_alteracoes = controlar;
        Ok(self.fim_da_edicao_sem_blocos())
    }

    /// O modo de agora: `leitura`, `sugerindo` ou `editando`.
    pub fn modo_edicao(&self) -> &'static str {
        if self.somente_leitura {
            "leitura"
        } else if self.doc.extras.controlar_alteracoes {
            "sugerindo"
        } else {
            "editando"
        }
    }

    /// Revisão → Proteger: o documento fica só para ler e comentar (gravado no arquivo), até desproteger.
    pub fn proteger(&mut self, sim: bool) -> Mudanca {
        if self.doc.extras.protegido == sim {
            return self.sem_mudanca();
        }
        self.doc.extras.protegido = sim;
        self.fim_da_edicao_sem_blocos()
    }

    /// Aceita (ou rejeita) as sugestões: da seleção, da que está no cursor ou (`tudo`) do documento todo.
    pub fn revisoes_resolver(&mut self, aceitar: bool, tudo: bool) -> Mudanca {
        if self.doc.blocos.is_empty() {
            return self.sem_mudanca();
        }
        let (mut t, ini, fim) = if tudo {
            let n = self.doc.blocos.len() - 1;
            (self.trabalho(0, n), self.inicio_do_bloco(0), self.fim_do_bloco(n))
        } else if self.cursor.vazio() {
            let Some((a, b)) = self.sugestao_no_cursor() else { return self.sem_mudanca() };
            let (t, _, _) = self.trabalho_da_selecao();
            (t, a, b)
        } else {
            self.trabalho_da_selecao()
        };
        let mut mudou = false;
        if tudo {
            for b in &mut t.blocos {
                mudou |= resolver_bloco(b, aceitar);
            }
        }
        for f in faixas(&t, ini, fim) {
            mudou |= crate::alvo::com_texto_ref(&mut t.blocos[f.bloco].conteudo, f.celula, |x| resolver(x, f.a, f.b, aceitar)).unwrap_or(false);
        }
        if !mudou {
            return self.sem_mudanca();
        }
        let cursor = Cursor::em(self.cursor.foco);
        self.sem_rastrear(|s| s.fechar_trabalho(t, cursor, None))
    }

    /// A sugestão contínua (mesmo tipo e autor) em volta do cursor: `(começo, fim)`.
    fn sugestao_no_cursor(&self) -> Option<(Pos, Pos)> {
        let f = self.cursor.foco;
        let t = texto_alvo(&self.doc.blocos[self.idx(f.bloco)?].conteudo, f.celula)?;
        let mut pos = 0;
        let mut achou: Option<(usize, usize, Revisao)> = None;
        for tr in &t {
            let n = tr.texto.chars().count();
            if let Some(r) = tr.rev.as_deref() {
                match &mut achou {
                    Some((_, b, rv)) if *b == pos && rv == r => *b = pos + n,
                    Some((a, b, _)) if f.desl >= *a && f.desl <= *b => break,
                    _ => achou = Some((pos, pos + n, r.clone())),
                }
            } else if achou.as_ref().is_some_and(|(a, b, _)| f.desl >= *a && f.desl <= *b) {
                break;
            } else {
                achou = None;
            }
            pos += n;
        }
        let (a, b, _) = achou.filter(|(a, b, _)| f.desl >= *a && f.desl <= *b)?;
        Some((Pos { desl: a, ..f }, Pos { desl: b, ..f }))
    }

    /// As sugestões do documento, na ordem (a lista lateral).
    pub fn revisoes(&self) -> Vec<InfoRevisao> {
        let mut v: Vec<InfoRevisao> = Vec::new();
        for b in &self.doc.blocos {
            for t in b.conteudo.textos() {
                // Só junta sugestões vizinhas: texto normal no meio separa uma da outra.
                let mut colado = false;
                for tr in t {
                    let Some(r) = tr.rev.as_deref() else {
                        colado = false;
                        continue;
                    };
                    match v.last_mut() {
                        Some(u) if colado && u.tipo == r.tipo && u.autor == r.autor => u.texto.push_str(&tr.texto),
                        _ => v.push(InfoRevisao { tipo: r.tipo, autor: r.autor.clone(), quando: r.quando.clone(), texto: tr.texto.clone(), bloco: b.id }),
                    }
                    colado = true;
                }
            }
        }
        v
    }

    /// Revisão → Novo comentário: liga `texto` à seleção (dentro de um parágrafo) ou à palavra do cursor.
    pub fn comentario_novo(&mut self, texto: &str) -> Result<Mudanca, String> {
        if texto.trim().is_empty() {
            return Err("Escreva o comentário.".to_string());
        }
        let (ini, mut fim) = self.selecao_ordenada();
        let i = self.idx(ini.bloco).ok_or("O cursor está fora do texto.")?;
        let alvo = texto_alvo(&self.doc.blocos[i].conteudo, ini.celula).ok_or("Aqui não tem texto para comentar.")?;
        if fim.bloco != ini.bloco || fim.celula != ini.celula {
            fim = Pos { desl: tamanho(&alvo), ..ini };
        }
        let (a, b) = if ini.desl == fim.desl {
            let chars: Vec<char> = plano(&alvo).chars().collect();
            crate::texto::palavra_em(&chars, ini.desl)
        } else {
            (ini.desl, fim.desl)
        };
        let id = self.doc.comentarios.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let mut bloco = self.doc.blocos[i].clone();
        crate::alvo::com_texto_ref(&mut bloco.conteudo, ini.celula, |x| {
            crate::texto::inserir_objeto(x, b, Marca::Comentario { id, fim: true });
            crate::texto::inserir_objeto(x, a, Marca::Comentario { id, fim: false });
        })
        .ok_or("Aqui não tem texto para comentar.")?;
        let r = self.revisao(TipoDeRevisao::Insercao);
        self.doc.comentarios.push(Comentario { trechos: Vec::new(), iniciais: String::new(), id, autor: r.autor, quando: r.quando, texto: texto.trim().to_string(), resolvido: false, respostas: Vec::new() });
        let cursor = Cursor::em(Pos { desl: b + 2, ..ini });
        Ok(self.liberado(|s| s.sem_rastrear(|s| s.transacao(Splice { pos: i, quantos: 1, dentro: vec![bloco] }, cursor, None))))
    }

    fn comentario_mut(&mut self, id: u64) -> Result<&mut Comentario, String> {
        self.doc.comentarios.iter_mut().find(|c| c.id == id).ok_or_else(|| "O comentário não existe mais.".to_string())
    }

    /// Responder, corrigir o texto ou marcar como resolvido (`None` deixa como está).
    pub fn comentario_mudar(&mut self, id: u64, texto: Option<&str>, resposta: Option<&str>, resolvido: Option<bool>) -> Result<Mudanca, String> {
        let r = self.revisao(TipoDeRevisao::Insercao);
        let c = self.comentario_mut(id)?;
        if let Some(t) = texto.filter(|t| !t.trim().is_empty()) {
            c.texto = t.trim().to_string();
        }
        if let Some(t) = resposta.filter(|t| !t.trim().is_empty()) {
            c.respostas.push(Resposta { autor: r.autor, quando: r.quando, texto: t.trim().to_string() });
        }
        if let Some(v) = resolvido {
            c.resolvido = v;
        }
        Ok(self.fim_da_edicao_sem_blocos())
    }

    /// Apaga o comentário e as marcas dele no texto (um passo de desfazer).
    pub fn comentario_excluir(&mut self, id: u64) -> Mudanca {
        self.doc.comentarios.retain(|c| c.id != id);
        let tem = |b: &crate::modelo::Bloco| b.conteudo.textos().into_iter().flatten().any(|t| matches!(t.marca.as_deref(), Some(Marca::Comentario { id: x, .. }) if *x == id));
        let (Some(i0), Some(i1)) = (self.doc.blocos.iter().position(tem), self.doc.blocos.iter().rposition(tem)) else { return self.fim_da_edicao_sem_blocos() };
        let mut t: Trabalho = self.trabalho(i0, i1);
        for b in &mut t.blocos {
            for x in b.conteudo.textos_mut() {
                x.retain(|tr| !matches!(tr.marca.as_deref(), Some(Marca::Comentario { id: y, .. }) if *y == id));
                normalizar(x);
            }
        }
        let cursor = Cursor::em(self.cursor.foco);
        self.liberado(|s| s.sem_rastrear(|s| s.fechar_trabalho(t, cursor, None)))
    }

    /// Roda `f` mesmo com o documento travado (comentar é permitido na leitura e no protegido).
    fn liberado<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let antes = self.comentando;
        self.comentando = true;
        let r = f(self);
        self.comentando = antes;
        r
    }

    /// Onde está o comentário: o bloco, a célula e o trecho `[a, b)`.
    fn trecho_do_comentario(&self, id: u64) -> Option<(usize, Cel, usize, usize)> {
        for (i, b) in self.doc.blocos.iter().enumerate() {
            let cels: Vec<Cel> = match &b.conteudo {
                Conteudo::Tabela { linhas, .. } => (0..linhas.len()).flat_map(|l| (0..linhas[l].len()).map(move |c| Some((l as u32, c as u32)))).collect(),
                _ => vec![None],
            };
            for cel in cels {
                let Some(t) = texto_alvo(&b.conteudo, cel) else { continue };
                let (mut a, mut pos) = (None, 0);
                for tr in &t {
                    match tr.marca.as_deref() {
                        Some(Marca::Comentario { id: x, fim: false }) if *x == id => a = Some(pos + 1),
                        Some(Marca::Comentario { id: x, fim: true }) if *x == id => return Some((i, cel, a.unwrap_or(pos), pos)),
                        _ => {}
                    }
                    pos += tr.texto.chars().count();
                }
            }
        }
        None
    }

    /// Os comentários que ainda estão no texto, na ordem do documento.
    pub fn comentarios(&self) -> Vec<InfoComentario> {
        let mut v: Vec<InfoComentario> = self
            .doc
            .comentarios
            .iter()
            .filter_map(|c| {
                let (i, cel, a, b) = self.trecho_do_comentario(c.id)?;
                let bloco = &self.doc.blocos[i];
                let trecho = texto_alvo(&bloco.conteudo, cel).map(|t| texto_puro(&fatia(&t, a, b))).unwrap_or_default();
                let trecho = if trecho.chars().count() > 60 { format!("{}…", trecho.chars().take(59).collect::<String>()) } else { trecho };
                Some(InfoComentario { id: c.id, autor: c.autor.clone(), quando: c.quando.clone(), texto: c.texto.clone(), resolvido: c.resolvido, respostas: c.respostas.clone(), trecho, bloco: bloco.id, pagina: self.pagina_do_caractere(i, a) })
            })
            .collect();
        v.sort_by_key(|c| (c.pagina, self.idx(c.bloco)));
        v
    }

    /// Seleciona o trecho comentado.
    pub fn comentario_ir(&mut self, id: u64) -> bool {
        let Some((i, celula, a, b)) = self.trecho_do_comentario(id) else { return false };
        let bloco = self.doc.blocos[i].id;
        self.cursor = Cursor { ancora: Pos { bloco, celula, desl: a }, foco: Pos { bloco, celula, desl: b }, x_desejado: None, pendente: None };
        true
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::cursor::Movimento;
    use crate::modelo::Documento;

    fn sessao(textos: &[&str]) -> Sessao {
        let mut d = Documento::vazio();
        for t in textos {
            d.acrescentar(Conteudo::Paragrafo { texto: crate::inline::analisar(t) });
        }
        let mut s = Sessao::com_documento(d);
        s.autor = "Ana".into();
        s
    }

    fn texto(s: &Sessao, i: usize) -> Vec<Trecho> {
        s.documento().blocos[i].conteudo.textos()[0].clone()
    }

    /// O texto com as marcas: `[+x]` inserido, `[-x]` apagado.
    fn marcado(t: &[Trecho]) -> String {
        t.iter()
            .map(|x| match x.rev.as_deref().map(|r| r.tipo) {
                Some(TipoDeRevisao::Insercao) => format!("[+{}]", x.texto),
                Some(TipoDeRevisao::Remocao) => format!("[-{}]", x.texto),
                Some(TipoDeRevisao::Formatacao) => format!("[*{}]", x.texto),
                None => x.texto.clone(),
            })
            .collect()
    }

    fn ir(s: &mut Sessao, desl: usize) {
        let id = s.documento().blocos[0].id;
        s.cursor = Cursor::em(Pos { bloco: id, celula: None, desl });
    }

    #[test]
    fn delete_em_sugestao_anda_pelo_texto() {
        let mut s = sessao(&["abcd"]);
        s.modo_edicao_definir("sugerindo").unwrap();
        ir(&mut s, 0);
        for _ in 0..3 {
            s.apagar(false, false);
        }
        assert_eq!(marcado(&texto(&s, 0)), "[-abc]d");
    }

    #[test]
    fn digitar_e_apagar_viram_sugestoes_e_aceitar_rejeitar_resolvem() {
        let mut s = sessao(&["um dois tres"]);
        s.modo_edicao_definir("sugerindo").unwrap();
        ir(&mut s, 3);
        s.inserir_texto("novo ");
        assert_eq!(marcado(&texto(&s, 0)), "um [+novo ]dois tres");
        ir(&mut s, 13);
        s.tecla("Backspace", false, false, false);
        assert_eq!(marcado(&texto(&s, 0)), "um [+novo ]dois[- ]tres");
        assert_eq!(s.cursor.foco.desl, 12, "o cursor fica antes do riscado");
        ir(&mut s, 5);
        s.tecla("Backspace", false, false, false);
        assert_eq!(marcado(&texto(&s, 0)), "um [+nvo ]dois[- ]tres", "apagar a própria inserção apaga de verdade");
        assert_eq!(s.revisoes().len(), 2, "a inserção e a remoção, separadas por texto normal");
        s.revisoes_resolver(true, true);
        assert_eq!(marcado(&texto(&s, 0)), "um nvo doistres");
        s.desfazer();
        ir(&mut s, 4);
        s.revisoes_resolver(false, false);
        assert_eq!(marcado(&texto(&s, 0)), "um dois[- ]tres", "rejeitar só a do cursor");
    }

    #[test]
    fn selecao_entre_paragrafos_e_riscada_e_nao_some() {
        let mut s = sessao(&["abc", "def"]);
        s.modo_edicao_definir("sugerindo").unwrap();
        ir(&mut s, 1);
        s.mover(Movimento::Baixo, true);
        s.apagar_selecao();
        assert_eq!(s.documento().blocos.len(), 2);
        assert_eq!(marcado(&texto(&s, 0)), "a[-bc]");
        assert_eq!(marcado(&texto(&s, 1)), "[-d]ef");
    }

    #[test]
    fn somente_leitura_e_protegido_travam_a_edicao() {
        let mut s = sessao(&["abc"]);
        s.modo_edicao_definir("leitura").unwrap();
        ir(&mut s, 3);
        s.inserir_texto("x");
        assert_eq!(marcado(&texto(&s, 0)), "abc");
        s.modo_edicao_definir("editando").unwrap();
        s.proteger(true);
        s.inserir_texto("x");
        assert_eq!(marcado(&texto(&s, 0)), "abc");
        s.proteger(false);
        s.inserir_texto("x");
        assert_eq!(marcado(&texto(&s, 0)), "abcx");
    }

    #[test]
    fn comentario_na_selecao_responder_resolver_e_excluir() {
        let mut s = sessao(&["o gato preto"]);
        ir(&mut s, 2);
        s.mover(Movimento::PalavraDireita, true);
        s.comentario_novo("Que gato?").unwrap();
        let c = s.comentarios();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].trecho.trim(), "gato");
        assert_eq!(c[0].autor, "Ana");
        s.comentario_mudar(c[0].id, None, Some("O do vizinho."), Some(true)).unwrap();
        let c = s.comentarios();
        assert!(c[0].resolvido && c[0].respostas.len() == 1);
        assert!(s.comentario_ir(c[0].id));
        assert_eq!(crate::inline::texto_puro(&texto(&s, 0)), "o gato preto");
        s.comentario_excluir(c[0].id);
        assert!(s.comentarios().is_empty());
        assert_eq!(texto(&s, 0).len(), 1, "as marcas saíram");
        // Sem seleção, o comentário vai na palavra do cursor.
        ir(&mut s, 9);
        s.comentario_novo("cor").unwrap();
        assert_eq!(s.comentarios()[0].trecho, "preto");
    }
}
