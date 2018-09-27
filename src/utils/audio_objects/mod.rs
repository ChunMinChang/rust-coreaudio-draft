extern crate coreaudio_sys;

mod audio_object_utils;
mod string_wrapper;

use self::string_wrapper::StringRef;
use self::coreaudio_sys::{
    kAudioObjectPropertyName,
    kAudioHardwarePropertyDevices,
    kAudioHardwarePropertyDefaultInputDevice,
    kAudioHardwarePropertyDefaultOutputDevice,
    kAudioDevicePropertyStreams,
    kAudioDevicePropertyDataSource,
    kAudioDevicePropertyDataSourceNameForIDCFString,
    kAudioObjectPropertyScopeInput,
    kAudioObjectPropertyScopeOutput,
    kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyElementMaster,
    AudioObjectPropertyAddress,
    AudioObjectID,
    kAudioObjectSystemObject,   // AudioObjectID
    kAudioObjectUnknown,        // AudioObjectID
    AudioStreamID,              // AudioObjectID
    AudioValueTranslation,
};
use std::fmt; // For fmt::{Debug, Formatter, Result}
use std::mem; // For mem::{uninitialized(), size_of()}
use std::os::raw::c_void;
use std::ptr; // For ptr::null()

// TODO: Move this const values to a shared module.
const DEVICE_NAME_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioObjectPropertyName,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

const DEVICE_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

const DEFAULT_INPUT_DEVICE_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

const DEFAULT_OUTPUT_DEVICE_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

const INPUT_DEVICE_STREAMS_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyStreams,
        mScope: kAudioObjectPropertyScopeInput,
        mElement: kAudioObjectPropertyElementMaster,
    };

const OUTPUT_DEVICE_STREAMS_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyStreams,
        mScope: kAudioObjectPropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMaster,
    };

const INPUT_DEVICE_SOURCE_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDataSource,
        mScope: kAudioObjectPropertyScopeInput,
        mElement: kAudioObjectPropertyElementMaster,
    };

const OUTPUT_DEVICE_SOURCE_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDataSource,
        mScope: kAudioObjectPropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMaster,
    };

const INPUT_DEVICE_SOURCE_NAME_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDataSourceNameForIDCFString,
        mScope: kAudioObjectPropertyScopeInput,
        mElement: kAudioObjectPropertyElementMaster,
    };

const OUTPUT_DEVICE_SOURCE_NAME_PROPERTY_ADDRESS: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDataSourceNameForIDCFString,
        mScope: kAudioObjectPropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMaster,
    };

// TODO: Maybe we should move this enum out since other module may also
//       need the scope.
// Using PartialEq for comparison.
#[derive(PartialEq)]
pub enum Scope {
    Input,
    Output,
}

// Using PartialEq for comparison.
#[derive(PartialEq)]
pub enum Error {
    ConversionFailed(string_wrapper::Error),
    InvalidParameters(audio_object_utils::Error),
    NoDeviceFound,
    SetSameDevice,
    WrongScope,
}

// To convert an audio_object_utils::Error to a Error.
impl From<audio_object_utils::Error> for Error {
    fn from(e: audio_object_utils::Error) -> Error {
        Error::InvalidParameters(e)
    }
}

// To convert an string_wrapper::Error to a Error.
impl From<string_wrapper::Error> for Error {
    fn from(e: string_wrapper::Error) -> Error {
        Error::ConversionFailed(e)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            Error::ConversionFailed(e) => format!("Fail to convert string: {:?}", e),
            Error::InvalidParameters(e) => format!("Invalid parameters: {:?}", e),
            Error::NoDeviceFound => "No valid device found by given information.".to_string(),
            Error::SetSameDevice => "Try setting the device with the same one".to_string(),
            Error::WrongScope => "The given scope is wrong.".to_string(),
        };
        write!(f, "{}", printable)
    }
}

// TODO: Move `AudioSystemObject`, `AudioObject` to independent module.

// AudioSystemObject
// ============================================================================
pub struct AudioSystemObject(AudioObjectID);

impl AudioSystemObject {
    pub fn new() -> Self {
        AudioSystemObject(kAudioObjectSystemObject)
    }

    pub fn get_default_device(
        &self,
        scope: &Scope
    ) -> Result<AudioObject, Error> {
        let address: &AudioObjectPropertyAddress = if scope == &Scope::Input {
            &DEFAULT_INPUT_DEVICE_PROPERTY_ADDRESS
        } else {
            &DEFAULT_OUTPUT_DEVICE_PROPERTY_ADDRESS
        };
        let device: AudioObject = self.get_property_data(address)?;
        // We will get an unknow device when there is no available device at
        // this time
        if device.is_valid() {
            Ok(device)
        } else {
            Err(Error::NoDeviceFound)
        }
    }

    // Apple has no API to get input-only or output-only devices. To do that,
    // we need to get all the devices first ans then check if they are input
    // or output one by one.
    pub fn get_devices(
        &self,
        scope: &Scope
    ) -> Result<Vec<AudioObject>, Error> {
        let mut devices: Vec<AudioObject> = self.get_all_devices()?;
        // It's ok to call `unwrap()` here since all the `AudioObjectID` values
        // in `devices` are valid.
        devices.retain(|ref device| device.in_scope(scope).unwrap());
        Ok(devices)
    }

    pub fn get_all_devices(&self) -> Result<Vec<AudioObject>, Error> {
        self.get_property_array::<AudioObject>(
            &DEVICE_PROPERTY_ADDRESS,
        ).map_err(|e| e.into())
    }

    pub fn set_default_device(
        &self,
        device: &AudioObject,
        scope: &Scope
    ) -> Result<(), Error> {
        // Surprisingly it's ok to set
        //   1. a unknown device
        //   2. a non-input/non-output device
        //   3. the current default input/output device
        // as the new default input/output device by apple's API.
        // We need to check the above things by ourselves.
        if !device.in_scope(scope)? {
            return Err(Error::WrongScope);
        }

        let default_device = self.get_default_device(scope)?;
        if device == &default_device {
            return Err(Error::SetSameDevice);
        }

        let address: &AudioObjectPropertyAddress = if scope == &Scope::Input {
            &DEFAULT_INPUT_DEVICE_PROPERTY_ADDRESS
        } else {
            &DEFAULT_OUTPUT_DEVICE_PROPERTY_ADDRESS
        };
        self.set_property_data(address, device.into()).map_err(|e| e.into())
    }

    fn get_property_data<T: Default>(
        &self,
        address: &AudioObjectPropertyAddress,
    ) -> Result<T, Error> {
        audio_object_utils::get_property_data::<T>(
            self.0,
            address
        ).map_err(|e| e.into())
    }

    fn get_property_array<T>(
        &self,
        address: &AudioObjectPropertyAddress,
    ) -> Result<Vec<T>, Error> {
        audio_object_utils::get_property_array::<T>(
            self.0,
            address
        ).map_err(|e| e.into())
    }

    fn set_property_data<T>(
        &self,
        address: &AudioObjectPropertyAddress,
        data: &T,
    ) -> Result<(), Error> {
        audio_object_utils::set_property_data(
            self.0,
            address,
            data
        ).map_err(|e| e.into())
    }
}

// AudioObject
// ============================================================================
#[derive(Clone, Debug, PartialEq)]
pub struct AudioObject(AudioObjectID);

impl AudioObject {
    pub fn new(id: AudioObjectID) -> Self {
        AudioObject(id)
    }

    pub fn is_valid(&self) -> bool {
        self.0 != kAudioObjectUnknown
    }

    pub fn get_device_label(
        &self,
        scope: &Scope
    ) -> Result<String, Error> {
        // Some USB headset(e.g., Plantronics .Audio 628) fails to get its
        // source. In that case, we return device name instead.
        match self.get_device_source_name(scope) {
            Ok(name) => Ok(name),
            Err(Error::WrongScope) => Err(Error::WrongScope),
            Err(_) => self.get_device_name(),
        }
    }

    pub fn get_device_name(&self) -> Result<String, Error> {
        // The size of `StringRef` is same as the size of `CFStringRef`, so the
        // queried data of `CFStringRef` can be stored into the memory of a
        // `CFStringRef` variable directly.
        // If the calling fails, the StringRef::drop() will be called but
        // nothing will be released since StringRef::Default::default() is a
        // null string.
        let name: StringRef =
            self.get_property_data(&DEVICE_NAME_PROPERTY_ADDRESS)?;
        name.into_string().map_err(Error::ConversionFailed)
    }

    pub fn get_device_source_name(
        &self,
        scope: &Scope
    ) -> Result<String, Error> {
        let mut source: u32 = self.get_device_source(scope)?;
        let mut name: StringRef = StringRef::new(ptr::null());

        let mut translation: AudioValueTranslation = AudioValueTranslation {
            mInputData: &mut source as *mut u32 as *mut c_void,
            mInputDataSize: mem::size_of::<u32>() as u32,
            mOutputData: &mut name as *mut StringRef as *mut c_void,
            mOutputDataSize: mem::size_of::<StringRef>() as u32,
        };

        let address: &AudioObjectPropertyAddress = if scope == &Scope::Input {
            &INPUT_DEVICE_SOURCE_NAME_PROPERTY_ADDRESS
        } else {
            &OUTPUT_DEVICE_SOURCE_NAME_PROPERTY_ADDRESS
        };

        self.get_property_data_with_ptr(address, &mut translation)?;
        name.into_string().map_err(Error::ConversionFailed)
    }

    fn get_device_source(
        &self,
        scope: &Scope
    ) -> Result<u32, Error> {
        if !self.in_scope(scope)? {
            return Err(Error::WrongScope);
        }

        let address: &AudioObjectPropertyAddress = if scope == &Scope::Input {
            &INPUT_DEVICE_SOURCE_PROPERTY_ADDRESS
        } else {
            &OUTPUT_DEVICE_SOURCE_PROPERTY_ADDRESS
        };
        self.get_property_data::<u32>(address).map_err(|e| e.into())
    }

    pub fn in_scope(
        &self,
        scope: &Scope
    ) -> Result<bool, Error> {
        let streams = self.number_of_streams(scope)?;
        Ok(streams > 0)
    }

    fn number_of_streams(
        &self,
        scope: &Scope
    ) -> Result<usize, Error> {
        let address: &AudioObjectPropertyAddress = if scope == &Scope::Input {
            &INPUT_DEVICE_STREAMS_PROPERTY_ADDRESS
        } else {
            &OUTPUT_DEVICE_STREAMS_PROPERTY_ADDRESS
        };
        let size = self.get_property_data_size(address)?;
        Ok(size / mem::size_of::<AudioStream>())
    }

    fn get_property_data<T: Default>(
        &self,
        address: &AudioObjectPropertyAddress,
    ) -> Result<T, Error> {
        audio_object_utils::get_property_data::<T>(
            self.0,
            address
        ).map_err(|e| e.into())
    }

    fn get_property_data_with_ptr<T>(
        &self,
        address: &AudioObjectPropertyAddress,
        data: &mut T,
    ) -> Result<(), Error> {
        audio_object_utils::get_property_data_with_ptr(
            self.0,
            address,
            data
        ).map_err(|e| e.into())
    }

    fn get_property_data_size(
        &self,
        address: &AudioObjectPropertyAddress,
    ) -> Result<usize, Error> {
        audio_object_utils::get_property_data_size(
            self.0,
            address
        ).map_err(|e| e.into())
    }
}

impl Default for AudioObject {
    fn default() -> Self {
        AudioObject::new(kAudioObjectUnknown)
    }
}

// AudioStream
// ============================================================================
struct AudioStream(AudioStreamID);

// Tests
// ============================================================================
#[cfg(test)]
mod test;