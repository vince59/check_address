use clap::Parser;
use csv::{ReaderBuilder, WriterBuilder};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;

/// Arguments ligne de commande
#[derive(Parser)]
#[command(name = "Adresse Checker", version)]
struct Args {
    /// Nom du fichier d'entrée (UTF-8, tabulation)
    input_file: String,

    /// Nombre de lignes à traiter (hors en-tête)
    lines_to_check: usize,
}

/// Structure pour lire les lignes du fichier
#[derive(Debug, Deserialize)]
struct InputRecord {
    nom: String,
    adresse: String,
    cp: String,
    ville: String,
    contact: String,
}

/// Structure pour écrire les lignes avec le champ en plus
#[derive(Debug, Serialize)]
struct OutputRecord {
    nom: String,
    adresse: String,
    cp: String,
    ville: String,
    contact: String,
    adresse_valide: bool,
}

/// Vérification de l'adresse via l'API publique
fn verifier_adresse_api(adresse: &str, cp: &str, ville: &str) -> bool {
    let client = reqwest::blocking::Client::new();
    let query = format!("{adresse}, {cp} {ville}");

    let mut url = Url::parse("https://api-adresse.data.gouv.fr/search/").unwrap();
    url.query_pairs_mut()
        .append_pair("q", &query)
        .append_pair("limit", "1");

    if let Ok(resp) = client.get(url).send() {
        if let Ok(json) = resp.json::<serde_json::Value>() {
            if let Some(features) = json.get("features") {
                if let Some(first) = features.get(0) {
                    if let Some(score) = first
                        .get("properties")
                        .and_then(|p| p.get("score"))
                        .and_then(|s| s.as_f64())
                    {
                        return score >= 0.7;
                    }
                }
            }
        }
    }
    false
}

/// Génération du nom de sortie avec suffixe _chk
fn generer_nom_sortie(input: &str) -> String {
    let path = Path::new(input);
    let stem = path.file_stem().unwrap().to_string_lossy();
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy())
        .unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| Path::new(""));

    let output_filename = format!("{}_chk.{}", stem, extension);
    parent.join(output_filename).to_string_lossy().to_string()
}

/// Fonction principale
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut rdr = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(&args.input_file)?;

    let output_path = generer_nom_sortie(&args.input_file);
    let mut wtr = WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(&output_path)?;

    // Écrire l’en-tête avec le champ supplémentaire
    wtr.write_record(&["nom", "adresse", "cp", "ville", "contact", "adresse_valide"])?;
    let pb = ProgressBar::new(args.lines_to_check as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} lignes ({eta})",
        )
        .unwrap(),
    );

    for (i, result) in rdr.deserialize::<InputRecord>().enumerate() {
        let input = result?;

        if i >= args.lines_to_check {
            break;
        }

        let ok = verifier_adresse_api(&input.adresse, &input.cp, &input.ville);
        std::thread::sleep(std::time::Duration::from_millis(33));

        let output = OutputRecord {
            nom: input.nom,
            adresse: input.adresse,
            cp: input.cp,
            ville: input.ville,
            contact: input.contact,
            adresse_valide: ok,
        };

        wtr.serialize(output)?;
        pb.inc(1);
    }
pb.finish_with_message("✔ Vérification terminée !");
    wtr.flush()?;
    println!("✅ Fichier généré : {}", output_path);
    Ok(())
}
