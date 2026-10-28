//! Adaptação de imagens do FeatherDocs para o motor compartilhado Image.
//! Os bytes originais permanecem no documento, inclusive durante a exportação
//! DOCX/FDOZ: apenas a prévia da tela passa por redimensionamento RGBA neutro.
use crate::localidade::{tr, trf};
use feather_image::ImagemRgba;
use image::{ImageFormat, ImageReader};
use std::io::Cursor;

const MAX_ARQUIVO: usize = 32 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;
// A página ocupa uns 800 px na tela a 100%: 1600 ainda fica nítido no zoom de 200%.
const LADO_PREVIA: u32 = 1_600;
const LARGURA_INSERCAO: f32 = 400.0;

/// Preserva os bytes PNG/JPEG em data URL, sem gravar o raster da prévia.
///
/// Valida dimensões a partir do cabeçalho ANTES de decodificar a imagem,
/// impedindo que a inserção aceite um arquivo comprimido com grade imensa.
pub fn preparar_insercao(bytes: &[u8]) -> Result<(String, f32, f32), String> {
    let (formato, largura, altura) = inspecionar(bytes)?;
    // O cabeçalho pode estar íntegro com dados comprimidos truncados.
    // A inserção precisa garantir que o conteúdo poderá ser exibido.
    ImageReader::with_format(Cursor::new(bytes), formato)
        .decode()
        .map_err(|e| trf("Imagem corrompida: {}", &[&e]))?;
    let mime = match formato {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        _ => return Err(tr("Formato de imagem não suportado.").into()),
    };
    let escala = (LARGURA_INSERCAO / largura as f32).min(1.0);
    let url = format!(
        "data:{mime};base64,{}",
        featherdocs_io::base64::codificar(bytes)
    );
    Ok((url, largura as f32 * escala, altura as f32 * escala))
}

fn inspecionar(bytes: &[u8]) -> Result<(ImageFormat, u32, u32), String> {
    if bytes.is_empty() || bytes.len() > MAX_ARQUIVO {
        return Err(tr("Imagem vazia ou maior que 32 MiB.").into());
    }
    let formato = image::guess_format(bytes)
        .map_err(|_| tr("Cabeçalho de imagem não reconhecido.").to_owned())?;
    if !matches!(formato, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(tr("Somente imagens PNG e JPEG são aceitas.").into());
    }
    let leitor = ImageReader::with_format(Cursor::new(bytes), formato);
    let (largura, altura) = leitor.into_dimensions()
        .map_err(|e| trf("Cabeçalho de imagem inválido: {}", &[&e]))?;
    if largura == 0 || altura == 0 || u64::from(largura) * u64::from(altura) > MAX_PIXELS {
        return Err(tr("Dimensões de imagem acima do limite seguro.").into());
    }
    Ok((formato, largura, altura))
}

/// O PNG/JPEG na resolução original, para o visualizador com zoom (a prévia da página para em
/// 1600 px e ficaria pixelada de perto). Confere as dimensões antes, como na inserção.
pub fn original_para_zoom(bytes: &[u8]) -> Option<(pelicano::Image, u32, u32)> {
    let (_, w, h) = inspecionar(bytes).ok()?;
    Some((crate::compat::imagem_de_bytes(bytes)?, w, h))
}

/// Decodifica os dados originais e cria pixels da prévia por Image compartilhado.
/// O redimensionamento bilinear é calculado com alfa pré-multiplicado, evitando
/// halos de cor em regiões transparentes. Nunca regrava o arquivo no documento.
pub fn preparar_previa(origem: &str) -> Option<(u32, u32, Vec<u8>)> {
    // Data URL expande ~4/3 em base64. Recuse antes de decodificar.
    const MAX_URL: usize = MAX_ARQUIVO * 4 / 3 + 128;
    if origem.len() > MAX_URL {
        return None;
    }
    let (mime, bytes) = featherdocs_io::base64::de_data_url(origem)?;
    if !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/jpg") {
        return None;
    }
    let (_, largura, altura) = inspecionar(&bytes).ok()?;
    let mut decodificada = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let maior = largura.max(altura);
    if maior > LADO_PREVIA {
        let escala = f64::from(LADO_PREVIA) / f64::from(maior);
        let w = (f64::from(largura) * escala).round().max(1.0) as u32;
        let h = (f64::from(altura) * escala).round().max(1.0) as u32;
        // O triângulo do `image` alarga com a redução (média da área): o bilinear de 4 pontos e a
        // GPU sem mipmap pulam pixels e serrilham o texto dos gráficos.
        decodificada = image::imageops::resize(&decodificada, w, h, image::imageops::FilterType::Triangle);
    }
    let saida = ImagemRgba::de_image(decodificada).ok()?;
    let (w, h) = saida.dimensoes();
    Some((w, h, saida.em_pixels()))
}

/// Aplica um dos mesmos efeitos do FeatherSheet a uma imagem PNG/JPEG embutida.
/// Retorna PNG data URL somente após sucesso, para que a sessão Docs atualize
/// o bloco como uma transação reversível (id, legenda e dimensões preservados).
/// SVG de gráficos, caminhos e URLs externas não são rasterizados implicitamente.
pub fn aplicar_efeito_embutido(
    origem: &str,
    efeito: feather_image::EfeitoRapido,
) -> Result<String, String> {
    use feather_image::EfeitoRapido;
    if efeito == EfeitoRapido::Original {
        return Ok(origem.to_owned());
    }
    const MAX_PIXELS_EFEITOS: u64 = 4_000_000;
    let (mime, bytes) = featherdocs_io::base64::de_data_url(origem)
        .ok_or_else(|| tr("A imagem não é um PNG/JPEG embutido válido.").to_string())?;
    if !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/jpg") {
        return Err(tr("O efeito exige uma imagem PNG/JPEG, não um gráfico SVG.").into());
    }
    let (_, largura, altura) = inspecionar(&bytes)?;
    if u64::from(largura) * u64::from(altura) > MAX_PIXELS_EFEITOS {
        return Err(tr("Imagem grande demais para edição interativa (4 milhões de pixels).").into());
    }
    let origem_rgba = ImagemRgba::de_image(
        image::load_from_memory(&bytes).map_err(|e| trf("Imagem corrompida: {}", &[&e]))?.to_rgba8()
    ).map_err(|e| trf("Buffer RGBA inválido: {}", &[&format!("{e:?}")]))?;
    let saida = efeito.executar(&origem_rgba)
        .map_err(|e| trf("Falha ao aplicar efeito: {}", &[&format!("{e:?}")]))?;
    let mut buffer = Vec::new();
    image::DynamicImage::ImageRgba8(saida.em_image())
        .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
        .map_err(|e| trf("Falha ao gerar PNG: {}", &[&e]))?;
    let data_url = format!(
        "data:image/png;base64,{}",
        featherdocs_io::base64::codificar(&buffer)
    );
    if data_url.len() > 16 * 1024 * 1024 {
        return Err(tr("PNG resultante excede o limite do documento (16 MiB).").into());
    }
    Ok(data_url)
}

/// Valida geometria de imagem de página em *pontos*, pela API neutra Drawing.
/// Não altera dimensões utilizadas pela composição, impressão ou exportação.
pub fn area_desenho(rect: [f32; 4]) -> Option<feather_draw::Retangulo> {
    let [x, y, w, h] = rect;
    if !rect.iter().all(|v| v.is_finite()) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let area = feather_draw::Retangulo::novo(x, y, w, h);
    Some(area)
}

#[cfg(test)]
mod testes {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::io::Cursor;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut buffer = Vec::new();
        let imagem = RgbaImage::from_fn(w, h, |x, y| match x {
            0 => Rgba([255, 0, 0, if y == 0 { 0 } else { 128 }]),
            _ => Rgba([0, 0, 255, 255]),
        });
        image::DynamicImage::ImageRgba8(imagem)
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png).unwrap();
        buffer
    }

    #[test]
    fn insercao_guarda_bytes_png_sem_modificar_fdoz_docx() {
        let original = png(2, 2);
        let (origem, largura, altura) = preparar_insercao(&original).unwrap();
        assert_eq!((largura, altura), (2.0, 2.0));
        let (mime, bytes) = featherdocs_io::base64::de_data_url(&origem).unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, original);
        let (w, h, pixels) = preparar_previa(&origem).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(&pixels[0..4], &[255, 0, 0, 0]);
        assert_eq!(&pixels[4..8], &[0, 0, 255, 255]);
    }

    #[test]
    fn alfa_sem_halo_no_motor_de_previa() {
        let entrada = ImagemRgba::nova(2, 1,
            vec![255, 0, 0, 255, 0, 255, 0, 0]).unwrap();
        let saida = entrada.redimensionar_bilinear(3, 1).unwrap();
        assert_eq!(saida.dimensoes(), (3, 1));
        assert_eq!(saida.pixels()[4], 255);
        assert_eq!(saida.pixels()[5], 0);
        assert_eq!(saida.pixels()[6], 0);
        assert_eq!(entrada.pixels(), &[255, 0, 0, 255, 0, 255, 0, 0]);
    }

    #[test]
    fn dimensoes_hostis_e_urls_externas_recusadas() {
        assert!(preparar_insercao(&[]).is_err());
        assert!(preparar_insercao(&vec![0; MAX_ARQUIVO + 1]).is_err());
        let mut corrompido = png(2, 2);
        corrompido[16..24].copy_from_slice(&[0, 1, 0, 0, 0, 1, 0, 0]);
        assert!(preparar_insercao(&corrompido).is_err());
        assert!(preparar_previa("https://exemplo.com/imagem.png").is_none());
        assert!(preparar_previa("file:///tmp/imagem.png").is_none());
    }

    #[test]
    fn img014_efeito_embutido_preserva_origem_e_alpha_e_recusa_url() {
        use feather_image::EfeitoRapido;
        let bytes = png(2, 2);
        let (origem, _, _) = preparar_insercao(&bytes).unwrap();
        let colorida = origem.clone();
        let processada = aplicar_efeito_embutido(&origem, EfeitoRapido::Cinza).unwrap();
        assert!(processada.starts_with("data:image/png;base64,"));
        assert_eq!(aplicar_efeito_embutido(&origem, EfeitoRapido::Original).unwrap(), colorida);
        let (_, dados) = featherdocs_io::base64::de_data_url(&processada).unwrap();
        let img = image::load_from_memory(&dados).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (2, 2));
        assert_eq!(img.get_pixel(0, 0).0[3], 0);
        assert_eq!(img.get_pixel(1, 0).0[3], 255);
        assert_eq!(img.get_pixel(1, 0).0[0], img.get_pixel(1, 0).0[1]);
        assert!(aplicar_efeito_embutido("https://example.com/img.png", EfeitoRapido::Cinza).is_err());
        assert!(aplicar_efeito_embutido("data:image/svg+xml;base64,AAA", EfeitoRapido::Cinza).is_err());
    }

    #[test]
    fn retangulos_validos_em_pontos_nao_dependem_de_dpi() {
        let r = area_desenho([10.0, 20.0, 240.0, 130.0]).unwrap();
        assert_eq!((r.x, r.y, r.largura, r.altura), (10.0, 20.0, 240.0, 130.0));
        assert!(area_desenho([0.0, 0.0, -1.0, 2.0]).is_none());
        assert!(area_desenho([f32::NAN, 0.0, 1.0, 2.0]).is_none());
    }
}
