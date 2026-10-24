use std::marker::PhantomData;
use std::sync::Mutex;

use serde::Serialize;
use wasapi::{DeviceEnumerator, Direction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioDeviceError {
    Unavailable,
    InvalidId,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInventory {
    pub devices: Vec<AudioDevice>,
    pub default_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSelection {
    pub devices: Vec<AudioDevice>,
    pub selected_id: Option<String>,
    pub selected_available: bool,
}

#[cfg_attr(test, mockall::automock)]
pub trait AudioDeviceCatalog: Send + Sync {
    fn scan(&self) -> Result<DeviceInventory, AudioDeviceError>;
}

/// Marcadores de tamanho zero: distinguem os serviços de saída (legendas)
/// e de entrada (ditado) no estado do Tauri, que é indexado por tipo.
pub struct Outputs;
pub struct Inputs;

pub struct DeviceService<Kind> {
    catalog: Box<dyn AudioDeviceCatalog>,
    selected_id: Mutex<Option<String>>,
    _kind: PhantomData<Kind>,
}

pub type OutputService = DeviceService<Outputs>;
pub type InputService = DeviceService<Inputs>;

impl<Kind> DeviceService<Kind> {
    pub fn new(catalog: Box<dyn AudioDeviceCatalog>, saved_id: Option<String>) -> Self {
        Self {
            catalog,
            selected_id: Mutex::new(saved_id),
            _kind: PhantomData,
        }
    }

    pub fn selection(&self) -> Result<DeviceSelection, AudioDeviceError> {
        let inventory = self.catalog.scan()?;
        let saved_id = self
            .selected_id
            .lock()
            .map_err(|_| AudioDeviceError::Unavailable)?
            .clone();
        let selected_id = saved_id.or(inventory.default_id);
        let selected_available = selected_id
            .as_ref()
            .is_some_and(|id| inventory.devices.iter().any(|device| &device.id == id));
        Ok(DeviceSelection {
            devices: inventory.devices,
            selected_id,
            selected_available,
        })
    }

    pub fn select(&self, id: &str) -> Result<DeviceSelection, AudioDeviceError> {
        if id.is_empty() || id.len() > 512 {
            return Err(AudioDeviceError::InvalidId);
        }
        let inventory = self.catalog.scan()?;
        if !inventory.devices.iter().any(|device| device.id == id) {
            return Err(AudioDeviceError::NotFound);
        }
        *self
            .selected_id
            .lock()
            .map_err(|_| AudioDeviceError::Unavailable)? = Some(id.to_owned());
        Ok(DeviceSelection {
            devices: inventory.devices,
            selected_id: Some(id.to_owned()),
            selected_available: true,
        })
    }
}

struct ComApartment {
    uninitialize: bool,
}

impl ComApartment {
    fn enter() -> Self {
        Self {
            uninitialize: wasapi::initialize_mta().is_ok(),
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.uninitialize {
            wasapi::deinitialize();
        }
    }
}

pub struct WasapiCatalog {
    direction: Direction,
}

impl WasapiCatalog {
    /// Saídas de reprodução (alto-falantes, fones) — fonte das legendas.
    pub fn outputs() -> Self {
        Self {
            direction: Direction::Render,
        }
    }

    /// Entradas de captura (microfones) — fonte do ditado.
    pub fn inputs() -> Self {
        Self {
            direction: Direction::Capture,
        }
    }
}

impl AudioDeviceCatalog for WasapiCatalog {
    fn scan(&self) -> Result<DeviceInventory, AudioDeviceError> {
        let _com = ComApartment::enter();
        let enumerator = DeviceEnumerator::new().map_err(|_| AudioDeviceError::Unavailable)?;
        let collection = enumerator
            .get_device_collection(&self.direction)
            .map_err(|_| AudioDeviceError::Unavailable)?;
        let devices = (&collection)
            .into_iter()
            .map(|item| {
                let device = item.map_err(|_| AudioDeviceError::Unavailable)?;
                Ok(AudioDevice {
                    id: device.get_id().map_err(|_| AudioDeviceError::Unavailable)?,
                    name: device
                        .get_friendlyname()
                        .map_err(|_| AudioDeviceError::Unavailable)?,
                })
            })
            .collect::<Result<Vec<_>, AudioDeviceError>>()?;
        let default_id = enumerator
            .get_default_device(&self.direction)
            .ok()
            .and_then(|device| device.get_id().ok());
        Ok(DeviceInventory {
            devices,
            default_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn selects_the_default_render_endpoint_without_using_its_name_as_identity() {
        let mut catalog = MockAudioDeviceCatalog::new();
        catalog.expect_scan().returning(|| {
            Ok(DeviceInventory {
                devices: vec![
                    device("device-a", "Alto-falantes"),
                    device("device-b", "Alto-falantes"),
                ],
                default_id: Some("device-b".into()),
            })
        });
        let service = OutputService::new(Box::new(catalog), None);
        let selection = service.selection().unwrap();
        assert_eq!(selection.selected_id.as_deref(), Some("device-b"));
        assert!(selection.selected_available);
        assert_eq!(selection.devices[0].name, selection.devices[1].name);
    }

    #[test]
    fn an_empty_inventory_has_no_selection() {
        let mut catalog = MockAudioDeviceCatalog::new();
        catalog.expect_scan().returning(|| {
            Ok(DeviceInventory {
                devices: vec![],
                default_id: None,
            })
        });
        let service = OutputService::new(Box::new(catalog), None);
        let selection = service.selection().unwrap();
        assert!(selection.devices.is_empty());
        assert_eq!(selection.selected_id, None);
        assert!(!selection.selected_available);
    }

    #[test]
    fn unavailable_saved_device_does_not_silently_switch_to_default() {
        let mut catalog = MockAudioDeviceCatalog::new();
        catalog.expect_scan().returning(|| {
            Ok(DeviceInventory {
                devices: vec![device("device-b", "Fones")],
                default_id: Some("device-b".into()),
            })
        });
        let service = OutputService::new(Box::new(catalog), Some("device-a".into()));
        assert_eq!(service.select("device-a"), Err(AudioDeviceError::NotFound));
        let selection = service.selection().unwrap();
        assert_eq!(selection.selected_id.as_deref(), Some("device-a"));
        assert!(!selection.selected_available);
    }

    #[test]
    fn explicit_selection_requires_an_active_output_id() {
        let mut catalog = MockAudioDeviceCatalog::new();
        catalog.expect_scan().returning(|| {
            Ok(DeviceInventory {
                devices: vec![device("device-a", "Fones"), device("device-b", "Monitor")],
                default_id: Some("device-a".into()),
            })
        });
        let service = OutputService::new(Box::new(catalog), None);
        assert_eq!(service.select(""), Err(AudioDeviceError::InvalidId));
        assert_eq!(service.select("device-c"), Err(AudioDeviceError::NotFound));
        let selection = service.select("device-b").unwrap();
        assert_eq!(selection.selected_id.as_deref(), Some("device-b"));
    }

    #[test]
    fn input_service_follows_the_same_selection_rules_as_outputs() {
        let mut catalog = MockAudioDeviceCatalog::new();
        catalog.expect_scan().returning(|| {
            Ok(DeviceInventory {
                devices: vec![device("mic-a", "Headset"), device("mic-b", "Webcam")],
                default_id: Some("mic-a".into()),
            })
        });
        let service = InputService::new(Box::new(catalog), None);
        let selection = service.selection().unwrap();
        assert_eq!(selection.selected_id.as_deref(), Some("mic-a"));
        assert!(selection.selected_available);
        let selection = service.select("mic-b").unwrap();
        assert_eq!(selection.selected_id.as_deref(), Some("mic-b"));
    }
}
