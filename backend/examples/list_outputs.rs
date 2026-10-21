use anycaptions_lib::audio::devices::{AudioDeviceCatalog, WasapiCatalog};

fn main() {
    match WasapiCatalog.scan() {
        Ok(inventory) => {
            if inventory.devices.is_empty() {
                println!("Nenhuma saída de reprodução ativa.");
            }
            for device in inventory.devices {
                let marker = if inventory.default_id.as_deref() == Some(&device.id) {
                    " (padrão)"
                } else {
                    ""
                };
                println!("{}{}", device.name, marker);
            }
        }
        Err(error) => eprintln!("Falha ao listar saídas: {error:?}"),
    }
}
