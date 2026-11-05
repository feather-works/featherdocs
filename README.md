# FeatherDocs

**The document editor of the FeatherWorks suite, written in Rust with a native
UI.** It writes, opens and saves `.docx`, LaTeX and Markdown, with the version
history kept inside the file itself, no account and no cloud.

> **Status:** alpha, in active development.

---

## What it does

| Area | Highlights |
|---|---|
| **Editing** | continuous editor (cursor and selection cross paragraphs), named styles and numbering, right-click menu, undo/redo in the engine |
| **Word** | reads and writes native `.docx` in Rust, no external converters; all 209 features of the parity test survive a round trip |
| **Templates** | 17 templates with shapes and illustrations, including five letterheads; **Files → Save as template** adds your own to the start page |
| **Layout** | text columns, watermarks, different first-page and even-page headers and footers |
| **LaTeX** | reads `.tex` with a parity test corpus, formulas through `feather-math`, a code panel beside the page and a settings tab (class, packages, preamble) |
| **Markdown** | edit and preview side by side |
| **References** | bibliography, fields and proofreading |
| **Writing aids** | Portuguese, English and Spanish spell checkers and Portuguese synonyms that run locally (LibreOffice dictionaries), also catching repeated words, double spaces and a space before punctuation; **Review → Language** marks text in another language, checked with that language's dictionary; correction as you type (curly quotes, dashes, capital at the start of a sentence), word goal in the status bar, word count per section in the Outline |
| **Tables from spreadsheets** | a range copied from FeatherSheet, Excel or LibreOffice pastes as a table, or fills the table at the cursor |
| **Before sending** | **Review → Check** lists what is still open: broken cross-references, citations without a source, open comments, pending suggestions, unknown words |
| **Reorganizing** | Alt+Shift+↑/↓ moves paragraphs; dragging a heading in the Outline moves the whole section |
| **Printing** | Ctrl+P prints straight to a system printer (CUPS `lp` on Linux and macOS); Ctrl+Shift+P exports the PDF |
| **Forms** | fills in Word forms: tick boxes, drop-down lists and date pickers are clicked like in Word and go back to `.docx` filled in |
| **Letters** | mail merge: `{{name}}` fields filled from a `.csv`, one copy per row |
| **Images and charts** | image effects and charts through the shared [`feather-charts`](https://github.com/feather-works/feather-charts) engine |
| **AI assistants** | `featherdocs-mcp`, an MCP server that lets assistants read and edit local documents; with **Review → AI assistant** on, it edits the document open in the app (each change is one Ctrl+Z) |

## A LaTeX interpreter of its own (work in progress)

FeatherDocs **does not call pdfLaTeX** or any other external program: it reads
the `.tex` and typesets the page by itself, in Rust. That is why the same
document can be edited on the page or in the code, side by side, and still be
real LaTeX.

![LaTeX code on the left and the page typeset by FeatherDocs on the right](docs/imagens/latex-codigo-e-papel.png)

The same page ([`docs/exemplos/vitrine.tex`](docs/exemplos/vitrine.tex)) in
pdfLaTeX and in FeatherDocs:

![The same page in pdfLaTeX and in FeatherDocs](docs/imagens/latex-lado-a-lado.png)

What the typesetter already handles:

- TeX paragraph breaking (Knuth-Plass, with the same penalties and the wider
  space after a period) and Portuguese hyphenation;
- formulas (`amsmath`) through [`feather-math`](crates/feather-math), with Latin Modern Math;
- tables with `booktabs`, `\multicolumn`, `\multirow`, `\rowcolors` and `longtable`;
- `tcolorbox`, `xcolor`, simple TikZ drawings (nodes, arrows, rectangles, circles);
- sections, lists, numbered captions, table of contents, bibliography (ABNT, APA, IEEE…), notes;
- common classes and packages (`article`, `exam`, `geometry`, `helvet`, `babel`).

**Not working yet, or only partly** (this is an early version):

- kerning: lines come out about 0.5% wider than in LaTeX and sometimes break
  in a different place;
- TikZ with curves, `\foreach` and `pgfplots`; node text is set with serifs;
- `multicols` longer than a page (shorter ones, and side-by-side `minipage`, come out side by side; longer ones stack and show a warning);
- spacing inside `tcolorbox` is a little larger than in LaTeX;
- `\textcite` citations and "et al.", the `\LaTeX` logo;
- features exclusive to LuaLaTeX/XeLaTeX (the reader follows the pdfLaTeX path).

In the test documents, about **68% of the lines** already break in exactly the
same place as in pdfLaTeX. Whatever the reader does not understand shows up in
the **Warnings** tab, with the source line.

Development notes (in Portuguese): what is missing for daily use
[`docs/PARIDADE-USO.md`](docs/PARIDADE-USO.md), fixes from the 18/10 audit
[`docs/PLANO-CORRECOES.md`](docs/PLANO-CORRECOES.md), plan for the features
still marked "Coming soon" [`docs/PLANO-EM-BREVE.md`](docs/PLANO-EM-BREVE.md),
LaTeX parity [`docs/PARIDADE-LATEX.md`](docs/PARIDADE-LATEX.md).

## The `.fdoz` format

A ZIP container with the data in Apache Arrow and an **embedded Git repository**
(built with [`gix`](https://github.com/Byron/gitoxide), pure Rust): the
document's versions live inside the file, no server needed. **File → Versions…**
lists them, compares one with the current text or restores it, and **Save named
version…** gives a save a name. When the history passes 64 MiB, the older half of
the versions is dropped so the file does not grow forever.

## Layout

| Crate | Role |
|---|---|
| [`featherdocs-core`](crates/featherdocs-core) | the document (block tree), deterministic pagination and editing with undo |
| [`featherdocs-io`](crates/featherdocs-io) | `.fdoz`, `.docx`, LaTeX and Markdown |
| [`featherdocs-historico`](crates/featherdocs-historico) | the Git history inside `.fdoz` |
| [`featherdocs-tabelas`](crates/featherdocs-tabelas) | insert/delete rows and columns, merge cells, through the shared `feather-tables` engine |
| [`feather-math`](crates/feather-math) | LaTeX formula typesetting with Latin Modern Math built in |
| [`featherdocs-mcp`](crates/featherdocs-mcp) | the MCP server for AI assistants |
| [`featherdocs-pelicano`](crates/featherdocs-pelicano) | the native app on [`pelicano`](https://github.com/feather-works/pelicano) (the one that ships) |

## Building

The repository builds on its own: the suite's shared libraries
([`feather-ui`](https://github.com/feather-works/feather-ui),
[`feather-draw`](https://github.com/feather-works/feather-draw),
[`feather-charts`](https://github.com/feather-works/feather-charts) and
[`feather-io`](https://github.com/feather-works/feather-io)) come in by git tag.

```sh
cd crates/featherdocs-pelicano
cargo run --release
```

Installers: `pacote/gerar.sh` (Linux `.deb` and `.tar.gz`, Windows `.exe`).

## Privacy

Everything runs locally. No document leaves your machine; there is no account,
login, telemetry or server.

## Documentation (in Portuguese)

- [Architecture](ARQUITETURA.md)
- [Menu specification](docs/MENU-ESPECIFICACAO.md)
- [Task log](docs/REGISTRO-DE-TAREFAS.md) — history of deliveries up to the repository split

## License

[GPL-3.0](LICENSE).
