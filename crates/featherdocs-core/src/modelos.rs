//! Modelos de documento da tela inicial ("Novo"): cada um monta um `Documento` de verdade,
//! com títulos, listas e tabelas do próprio motor (a miniatura é a primeira página dele).
use crate::inline::analisar;
use crate::modelo::{Alinhamento, Conteudo, Documento};

/// O que a galeria mostra de cada modelo.
pub struct ModeloInfo {
    pub id: &'static str,
    pub nome: &'static str,
    pub descricao: &'static str,
}

const MODELOS: [ModeloInfo; 6] = [
    ModeloInfo { id: "em-branco", nome: "Documento em branco", descricao: "Uma página A4 vazia para começar do zero." },
    ModeloInfo { id: "carta", nome: "Carta", descricao: "Data, destinatário, assunto, corpo e assinatura." },
    ModeloInfo { id: "relatorio", nome: "Relatório", descricao: "Capa simples, sumário, seções e uma tabela de resultados." },
    ModeloInfo { id: "curriculo", nome: "Currículo", descricao: "Contato, resumo, experiência, formação e habilidades." },
    ModeloInfo { id: "ata", nome: "Ata de reunião", descricao: "Participantes, pauta, decisões e próximos passos." },
    ModeloInfo { id: "academico", nome: "Trabalho acadêmico", descricao: "Capa, resumo, sumário e capítulos, no formato da ABNT." },
];

pub fn lista() -> &'static [ModeloInfo] {
    &MODELOS
}

/// Acumula blocos; `alinhado` muda o alinhamento do último.
struct Montagem(Documento);

impl Montagem {
    fn novo() -> Self {
        Self(Documento::vazio())
    }
    fn titulo(mut self, nivel: u8, t: &str) -> Self {
        self.0.acrescentar(Conteudo::Titulo { nivel, texto: analisar(t) });
        self
    }
    fn par(mut self, t: &str) -> Self {
        self.0.acrescentar(Conteudo::Paragrafo { texto: analisar(t) });
        self
    }
    fn item(mut self, ordenado: bool, t: &str) -> Self {
        self.0.acrescentar(Conteudo::Item { ordenado, nivel: 0, texto: analisar(t) });
        self
    }
    fn tabela(mut self, cabecalho: bool, linhas: &[&[&str]]) -> Self {
        let linhas = linhas.iter().map(|l| l.iter().map(|c| analisar(c)).collect()).collect();
        self.0.acrescentar(Conteudo::Tabela { cabecalho, linhas });
        self
    }
    fn bloco(mut self, c: Conteudo) -> Self {
        self.0.acrescentar(c);
        self
    }
    fn alinhado(mut self, a: Alinhamento) -> Self {
        if let Some(b) = self.0.blocos.last_mut() {
            b.atrib.alinhamento = a;
        }
        self
    }
    fn fim(mut self, titulo: &str) -> Documento {
        self.0.meta.titulo = titulo.into();
        self.0
    }
}

/// O documento do modelo `id`; `None` se não existe.
pub fn criar(id: &str) -> Option<Documento> {
    use Alinhamento::{Centro, Direita, Justificado};
    let d = match id {
        "em-branco" => Documento::vazio(),
        "carta" => Montagem::novo()
            .par("São Paulo, 28 de outubro de 2026").alinhado(Direita)
            .par("")
            .par("À **Empresa Exemplo Ltda.**")
            .par("Departamento de Compras")
            .par("Rua das Flores, 123 — São Paulo, SP")
            .par("")
            .par("**Assunto:** proposta de fornecimento")
            .par("")
            .par("Prezados senhores,")
            .par("Escreva aqui o motivo da carta. Diga em poucas linhas o que você precisa ou oferece, e o que espera como resposta.").alinhado(Justificado)
            .par("No segundo parágrafo, dê os detalhes: prazos, valores ou documentos anexos.").alinhado(Justificado)
            .par("")
            .par("Atenciosamente,")
            .par("")
            .par("Seu nome")
            .par("Cargo · telefone · e-mail")
            .fim("Carta"),
        "relatorio" => Montagem::novo()
            .titulo(1, "Relatório de atividades")
            .par("Equipe · Outubro de 2026")
            .bloco(Conteudo::sumario(2))
            .titulo(1, "Resumo")
            .par("Em um parágrafo, o que foi feito, o resultado principal e a recomendação.").alinhado(Justificado)
            .titulo(1, "Resultados")
            .par("Os números do período:")
            .tabela(true, &[&["Indicador", "Meta", "Realizado"], &["Atendimentos", "1.200", "1.348"], &["Prazo médio", "3 dias", "2,4 dias"], &["Satisfação", "90%", "93%"]])
            .titulo(2, "Destaques")
            .item(false, "Primeiro destaque do período.")
            .item(false, "Segundo destaque, com o impacto dele.")
            .titulo(1, "Próximos passos")
            .item(true, "Ação, responsável e prazo.")
            .item(true, "Outra ação.")
            .fim("Relatório de atividades"),
        "curriculo" => Montagem::novo()
            .titulo(1, "Seu Nome Completo").alinhado(Centro)
            .par("Cidade, UF · (11) 90000-0000 · voce@email.com · linkedin.com/in/voce").alinhado(Centro)
            .titulo(2, "Resumo")
            .par("Duas ou três linhas sobre a sua experiência e o que você procura.").alinhado(Justificado)
            .titulo(2, "Experiência")
            .par("**Cargo** — Empresa · 2023 – atual")
            .item(false, "Uma conquista com número: reduziu o prazo em 30%.")
            .item(false, "Outra responsabilidade importante.")
            .par("**Cargo anterior** — Empresa · 2020 – 2023")
            .item(false, "O que você fez e o resultado.")
            .titulo(2, "Formação")
            .par("**Curso** — Instituição · 2019")
            .titulo(2, "Habilidades")
            .item(false, "Ferramentas, idiomas e certificações.")
            .fim("Currículo"),
        "ata" => Montagem::novo()
            .titulo(1, "Ata de reunião")
            .tabela(false, &[&["Data", "28/10/2026, 14h"], &["Local", "Sala 2 / videochamada"], &["Participantes", "Ana, Bruno, Carla"], &["Redação", "Seu nome"]])
            .titulo(2, "Pauta")
            .item(true, "Primeiro assunto.")
            .item(true, "Segundo assunto.")
            .titulo(2, "Discussão e decisões")
            .par("O que foi conversado em cada item e o que ficou decidido.").alinhado(Justificado)
            .titulo(2, "Próximos passos")
            .tabela(true, &[&["Ação", "Responsável", "Prazo"], &["Enviar a proposta", "Ana", "04/11"], &["Revisar o orçamento", "Bruno", "07/11"]])
            .fim("Ata de reunião"),
        "academico" => Montagem::novo()
            .par("**UNIVERSIDADE EXEMPLO**").alinhado(Centro)
            .par("Curso de Exemplo").alinhado(Centro)
            .par("")
            .par("NOME DO AUTOR").alinhado(Centro)
            .par("")
            .titulo(1, "Título do trabalho: subtítulo").alinhado(Centro)
            .par("")
            .par("Cidade").alinhado(Centro)
            .par("2026").alinhado(Centro)
            .bloco(Conteudo::QuebraDePagina)
            .titulo(1, "Resumo")
            .par("Texto corrido de 150 a 500 palavras: objetivo, método, resultados e conclusões.").alinhado(Justificado)
            .par("**Palavras-chave:** palavra; outra palavra; mais uma.")
            .bloco(Conteudo::QuebraDePagina)
            .bloco(Conteudo::sumario(3))
            .bloco(Conteudo::QuebraDePagina)
            .titulo(1, "Introdução")
            .par("Apresente o tema, o problema e os objetivos do trabalho.").alinhado(Justificado)
            .titulo(1, "Desenvolvimento")
            .titulo(2, "Fundamentação teórica")
            .par("O que os autores já disseram sobre o tema.").alinhado(Justificado)
            .titulo(1, "Conclusão")
            .par("Retome os objetivos e diga o que foi alcançado.").alinhado(Justificado)
            .titulo(1, "Referências")
            .par("SOBRENOME, Nome. **Título da obra**. Cidade: Editora, ano.")
            .fim("Trabalho acadêmico"),
        _ => return None,
    };
    Some(d)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_modelo_da_lista_cria_um_documento_que_pagina() {
        for m in lista() {
            let d = criar(m.id).unwrap_or_else(|| panic!("{} não cria", m.id));
            let s = crate::sessao::Sessao::com_documento(d);
            assert!(s.total_de_paginas() >= 1, "{}", m.id);
            assert!(s.quadro(0, u64::MAX).is_ok(), "{}: a miniatura sai da primeira página", m.id);
        }
        assert!(criar("nao-existe").is_none());
    }
}
