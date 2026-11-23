#[derive(Clone, Copy, serde::Serialize, PartialEq, Eq)]
pub struct DataType {
    scale: u16,
    signed: bool,
    unit: Option<&'static str>,
}

#[derive(thiserror::Error, Debug)]
pub enum ParseValueError {
    #[error("could not parse integer value")]
    Integer(#[source] std::num::ParseIntError),
    #[error("could not parse floating point value")]
    Float(#[source] std::num::ParseFloatError),
    #[error("{0} has too much precision that cannot be represented")]
    FloatTooPrecise(f32),
    #[error("could not be converted to a register value")]
    OutOfRange(#[source] std::num::TryFromIntError),
}

impl DataType {
    // Convenience aliases for nicely tabulated `for_each_register` macro definition below.
    pub const U16: Self = Self { scale: 1, signed: false, unit: None };
    pub const I16: Self = Self { scale: 1, signed: true, unit: None };
    pub const CEL: Self = Self { scale: 10, signed: true, unit: Some("°C") };
    pub const SPH: Self = Self { scale: 10, signed: false, unit: Some("g/kg") };
    /// Tenths, without a unit.
    pub const DEC: Self = Self { scale: 10, signed: false, unit: None };
    /// Volts, stored in tenths.
    pub const DV: Self = Self { scale: 10, signed: false, unit: Some("V") };
    /// Volts, stored in thousandths.
    pub const MV: Self = Self { scale: 1000, signed: false, unit: Some("V") };
    pub const SEC: Self = Self { scale: 1, signed: false, unit: Some("s") };
    pub const PCT: Self = Self { scale: 1, signed: false, unit: Some("%") };
    pub const RPM: Self = Self { scale: 1, signed: false, unit: Some("rpm") };
    pub const PPM: Self = Self { scale: 1, signed: false, unit: Some("ppm") };

    pub fn from_bytes<'a>(self, mut bs: &'a [u8]) -> impl Iterator<Item = Value> + 'a {
        std::iter::from_fn(move || {
            let (v, remainder) = bs.split_first_chunk::<2>()?;
            bs = remainder;
            Some(self.from_word(u16::from_be_bytes(*v)))
        })
    }

    pub fn from_word(self, word: u16) -> Value {
        match (self.scale, self.signed) {
            (1, true) => Value::I16(word as i16),
            (1, false) => Value::U16(word),
            _ if self == Self::CEL => Value::Celsius(word as i16),
            _ if self == Self::SPH => Value::SpecificHumidity(word),
            (10, false) => Value::Tenths(word),
            (1000, false) => Value::Thousandths(word),
            _ => panic!("malformed DataType"),
        }
    }

    pub fn parse_string(self, string: &str) -> Result<Value, ParseValueError> {
        fn checked_float_convert<T: TryFrom<i64, Error = std::num::TryFromIntError>>(
            scale: f32,
            inp: &str,
        ) -> Result<T, ParseValueError> {
            let float = inp.parse::<f32>().map_err(ParseValueError::Float)?;
            let scaled = float * scale;
            if scaled.fract() != 0.0 {
                return Err(ParseValueError::FloatTooPrecise(float));
            }
            (scaled as i64).try_into().map_err(ParseValueError::OutOfRange)
        }
        let scale = self.scale() as f32;
        Ok(match (self.scale, self.signed) {
            (1, true) => Value::I16(string.parse().map_err(ParseValueError::Integer)?),
            (1, false) => Value::U16(string.parse().map_err(ParseValueError::Integer)?),
            _ if self == Self::CEL => Value::Celsius(checked_float_convert(scale, string)?),
            _ if self == Self::SPH => {
                Value::SpecificHumidity(checked_float_convert(scale, string)?)
            }
            (10, false) => Value::Tenths(checked_float_convert(scale, string)?),
            (1000, false) => Value::Thousandths(checked_float_convert(scale, string)?),
            _ => panic!("malformed DataType"),
        })
    }

    pub const fn is_signed(&self) -> bool {
        self.signed
    }
    pub const fn scale(&self) -> u16 {
        self.scale
    }
    pub const fn unit(&self) -> Option<&'static str> {
        self.unit
    }

    pub const fn bytes(&self) -> usize {
        2
    }
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.signed { "S/" } else { "U/" })?;
        f.write_fmt(format_args!("{}", self.scale))?;
        if let Some(unit) = self.unit {
            f.write_fmt(format_args!(" {unit}"))?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    U16(u16),
    I16(i16),
    /// This data type contains a value multiplied by 10.
    Celsius(i16),
    /// This data type contains a value multiplied by 10.
    SpecificHumidity(u16),
    /// This data type contains a value multiplied by 10.
    Tenths(u16),
    /// This data type contains a value multiplied by 1000.
    Thousandths(u16),
}

impl Value {
    #[allow(non_snake_case)]
    const fn CEL(val: i16) -> Self {
        Self::Celsius(val)
    }
    #[allow(non_snake_case)]
    const fn SPH(val: u16) -> Self {
        Self::SpecificHumidity(val)
    }
    #[allow(non_snake_case, dead_code)]
    const fn DEC(val: u16) -> Self {
        Self::Tenths(val)
    }
    #[allow(non_snake_case, dead_code)]
    const fn DV(val: u16) -> Self {
        Self::Tenths(val)
    }
    #[allow(non_snake_case, dead_code)]
    const fn MV(val: u16) -> Self {
        Self::Thousandths(val)
    }
    #[allow(non_snake_case)]
    const fn SEC(val: u16) -> Self {
        Self::U16(val)
    }
    #[allow(non_snake_case)]
    const fn PCT(val: u16) -> Self {
        Self::U16(val)
    }
    #[allow(non_snake_case)]
    const fn RPM(val: u16) -> Self {
        Self::U16(val)
    }
    #[allow(non_snake_case)]
    const fn PPM(val: u16) -> Self {
        Self::U16(val)
    }

    /// Bytes of the value.
    ///
    /// Should use sparingly, in case there come up values that aren't 16-bits.
    pub(crate) fn into_inner(self) -> u16 {
        match self {
            Value::U16(v) => v,
            Value::I16(v) => v as u16,
            Value::Celsius(v) => v as u16,
            Value::SpecificHumidity(v) => v,
            Value::Tenths(v) => v,
            Value::Thousandths(v) => v,
        }
    }
}

impl From<Value> for String {
    fn from(value: Value) -> String {
        value.to_string()
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Value::U16(n) => <u16 as std::fmt::Display>::fmt(&n, f),
            Value::I16(n) => <i16 as std::fmt::Display>::fmt(&n, f),
            Value::Celsius(n) => <f32 as std::fmt::Display>::fmt(&(n as f32 / 10.0), f),
            Value::SpecificHumidity(n) | Value::Tenths(n) => {
                <f32 as std::fmt::Display>::fmt(&(n as f32 / 10.0), f)
            }
            Value::Thousandths(n) => <f32 as std::fmt::Display>::fmt(&(n as f32 / 1000.0), f),
        }
    }
}

impl serde::Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Value::U16(n) => serializer.serialize_u16(n),
            Value::I16(n) => serializer.serialize_i16(n),
            Value::Celsius(n) => serializer.serialize_f32(n as f32 / 10.0),
            Value::SpecificHumidity(n) | Value::Tenths(n) => {
                serializer.serialize_f32(n as f32 / 10.0)
            }
            Value::Thousandths(n) => serializer.serialize_f32(n as f32 / 1000.0),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Mode(u8);

impl serde::Serialize for Mode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0 & Self::R.0 == 0 { "-" } else { "R" })?;
        f.write_str(if self.0 & Self::W.0 == 0 { "-" } else { "W" })?;
        Ok(())
    }
}

impl Mode {
    pub const R: Self = Self(1 << 0);
    pub const W: Self = Self(1 << 1);
    pub const RW: Self = Self(Self::R.0 | Self::W.0);
    const R_: Self = Self::R;

    pub fn is_writable(&self) -> bool {
        (self.0 & Self::W.0) != 0
    }
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct RegisterIndex(u16);

impl RegisterIndex {
    pub const fn from_address(address: u16) -> Option<RegisterIndex> {
        let address = address as usize;
        #[expect(clippy::if_same_then_else)]
        if ADDRESS_INDICES.len() <= address {
            None
        } else if ADDRESS_INDICES[address] == 0xFFFF {
            None
        } else {
            Some(RegisterIndex(ADDRESS_INDICES[address]))
        }
    }

    pub const fn from_name(name: &str) -> Option<RegisterIndex> {
        const fn str_eq(a: &str, b: &str) -> bool {
            let (ab, bb) = (a.as_bytes(), b.as_bytes());
            let alen = ab.len();
            if alen != bb.len() {
                return false;
            }
            let mut i = 0;
            while i < alen {
                if ab[i] != bb[i] {
                    return false;
                }
                i += 1;
            }
            true
        }
        let mut i = NAMES.len() as u16;
        while i > 0 {
            i -= 1;
            if str_eq(NAMES[i as usize], name) {
                return Some(RegisterIndex(i));
            }
        }
        None
    }

    pub const fn address(&self) -> u16 {
        ADDRESSES[self.0 as usize]
    }

    pub const fn name(&self) -> &'static str {
        NAMES[self.0 as usize]
    }

    pub const fn data_type(&self) -> DataType {
        DATA_TYPES[self.0 as usize]
    }

    pub const fn mode(&self) -> Mode {
        MODES[self.0 as usize]
    }

    pub const fn minimum_value(&self) -> Option<Value> {
        MINIMUM_VALUES[self.0 as usize]
    }

    pub const fn maximum_value(&self) -> Option<Value> {
        MAXIMUM_VALUES[self.0 as usize]
    }
}

macro_rules! for_each_register {
    ($m:ident) => {
        $m! {
            1001: U16, R_, "DEMC_RH_HIGHEST", min = 0, max = 100;
            1002: PPM, R_, "DEMC_CO2_HIGHEST", min = 0, max = 2000;
            1003: U16, R_, "DEMC_FAN_SPEED";
            1004: U16, R_, "DEMC_ACTIVE_CONTROLLER", min = 0, max = 1;
            1007: U16, R_, "DEMC_FAN_SPEED_EAF";
            1011: U16, R_, "DEMC_RH_PI_SP", min = 0, max = 100;
            1012: U16, R_, "DEMC_RH_PI_FEEDBACK", min = 0, max = 100;
            1019: U16, R_, "DEMC_RH_PI_OUTPUT";
            1021: PPM, R_, "DEMC_CO2_PI_SP", min = 0, max = 2000;
            1022: PPM, R_, "DEMC_CO2_PI_FEEDBACK", min = 0, max = 2000;
            1029: U16, R_, "DEMC_CO2_PI_OUTPUT";
            1031: U16, RW, "DEMC_RH_SETTINGS_PBAND", min = 1, max = 100;
            1032: SEC, RW, "DEMC_RH_SETTINGS_ITIME", min = 0, max = 120;
            1033: U16, RW, "DEMC_RH_SETTINGS_SP_SUMMER", min = 10, max = 100;
            1034: U16, RW, "DEMC_RH_SETTINGS_SP_WINTER", min = 10, max = 100;
            1035: U16, RW, "DEMC_RH_SETTINGS_ON_OFF", min = 0, max = 1;
            1036: U16, R_, "SUMMER_WINTER_RESTART_COUNTER_L";
            1037: U16, R_, "SUMMER_WINTER_RESTART_COUNTER_H";
            1038: U16, R_, "SUMMER_WINTER_STATE", min = 0, max = 2;
            1039: U16, R_, "SUMMER_WINTER", min = 0, max = 1;
            1040: U16, RW, "SUMMER_WINTER_T";
            1041: PPM, RW, "DEMC_CO2_SETTINGS_PBAND", min = 50, max = 2000;
            1042: SEC, RW, "DEMC_CO2_SETTINGS_ITIME", min = 0, max = 120;
            1043: PPM, RW, "DEMC_CO2_SETTINGS_SP", min = 100, max = 2000;
            1044: U16, RW, "DEMC_CO2_SETTINGS_ON_OFF", min = 0, max = 1;
            1061: U16, R_, "DEMC_ENABLED", min = 0, max = 1;
            1062: U16, R_, "DEMC_AUTO_MODE_SOURCE", min = 0, max = 3;
            1093: U16, R_, "CFG_STORAGE_TRANSFER_PROGRESS";
            1101: U16, RW, "USERMODE_HOLIDAY_TIME", min = 1, max = 365;
            1102: U16, RW, "USERMODE_AWAY_TIME", min = 1, max = 72;
            1103: U16, RW, "USERMODE_FIREPLACE_TIME", min = 1, max = 60;
            1104: U16, RW, "USERMODE_REFRESH_TIME", min = 1, max = 240;
            1105: U16, RW, "USERMODE_CROWDED_TIME", min = 1, max = 8;
            1111: U16, R_, "USERMODE_REMAINING_TIME_L";
            1112: U16, R_, "USERMODE_REMAINING_TIME_H";
            1121: U16, RW, "IAQ_SPEED_LEVEL_MIN", min = 2, max = 3;
            1122: U16, RW, "IAQ_SPEED_LEVEL_MAX", min = 3, max = 5;
            1123: U16, R_, "IAQ_LEVEL", min = 0, max = 2;
            1131: U16, RW, "USERMODE_MANUAL_AIRFLOW_LEVEL_SAF", min = 0, max = 4;
            1132: U16, RW, "USERMODE_MANUAL_AIRFLOW_LEVEL_EAF";
            1133: U16, RW, "USERMODE_AUTO_AIRFLOW_LEVEL_SAF";
            1134: U16, RW, "USERMODE_AUTO_AIRFLOW_LEVEL_EAF";
            1135: U16, RW, "USERMODE_CROWDED_AIRFLOW_LEVEL_SAF", min = 3, max = 5;
            1136: U16, RW, "USERMODE_CROWDED_AIRFLOW_LEVEL_EAF", min = 3, max = 5;
            1137: U16, RW, "USERMODE_REFRESH_AIRFLOW_LEVEL_SAF", min = 3, max = 5;
            1138: U16, RW, "USERMODE_REFRESH_AIRFLOW_LEVEL_EAF", min = 3, max = 5;
            1139: U16, RW, "USERMODE_FIREPLACE_AIRFLOW_LEVEL_SAF", min = 3, max = 5;
            1140: U16, RW, "USERMODE_FIREPLACE_AIRFLOW_LEVEL_EAF", min = 1, max = 3;
            1141: U16, RW, "USERMODE_AWAY_AIRFLOW_LEVEL_SAF", min = 0, max = 3;
            1142: U16, RW, "USERMODE_AWAY_AIRFLOW_LEVEL_EAF", min = 0, max = 3;
            1143: U16, RW, "USERMODE_HOLIDAY_AIRFLOW_LEVEL_SAF", min = 0, max = 3;
            1144: U16, RW, "USERMODE_HOLIDAY_AIRFLOW_LEVEL_EAF", min = 0, max = 3;
            1145: U16, RW, "USERMODE_COOKERHOOD_AIRFLOW_LEVEL_SAF", min = 1, max = 5;
            1146: U16, RW, "USERMODE_COOKERHOOD_AIRFLOW_LEVEL_EAF", min = 1, max = 5;
            1147: U16, RW, "USERMODE_VACUUMCLEANER_AIRFLOW_LEVEL_SAF", min = 1, max = 5;
            1148: U16, RW, "USERMODE_VACUUMCLEANER_AIRFLOW_LEVEL_EAF", min = 1, max = 5;
            1151: CEL, RW, "USERMODE_CROWDED_T_OFFSET", min = -100, max = 0;
            1161: U16, R_, "USERMODE_MODE", min = 0, max = 12;
            1162: U16, RW, "USERMODE_HMI_CHANGE_REQUEST", min = 0, max = 7;
            1164: U16, RW, "USERMODE_FALLBACK", min = 0, max = 1;
            1165: U16, R_, "USERMODE_MODE_HMI";
            1171: U16, RW, "CDI_1_AIRFLOW_LEVEL_SAF", min = 0, max = 5;
            1172: U16, RW, "CDI_1_AIRFLOW_LEVEL_EAF", min = 0, max = 5;
            1173: U16, RW, "CDI_2_AIRFLOW_LEVEL_SAF", min = 0, max = 5;
            1174: U16, RW, "CDI_2_AIRFLOW_LEVEL_EAF", min = 0, max = 5;
            1175: U16, RW, "CDI_3_AIRFLOW_LEVEL_SAF", min = 0, max = 5;
            1176: U16, RW, "CDI_3_AIRFLOW_LEVEL_EAF", min = 0, max = 5;
            1177: U16, RW, "PRESSURE_GUARD_AIRFLOW_LEVEL_SAF", min = 0, max = 5;
            1178: U16, RW, "PRESSURE_GUARD_AIRFLOW_LEVEL_EAF", min = 0, max = 5;
            1181: U16, RW, "USERMODE_HOLIDAY_DI_OFF_DELAY", min = 0, max = 365;
            1182: U16, RW, "USERMODE_AWAY_DI_OFF_DELAY", min = 0, max = 72;
            1183: U16, RW, "USERMODE_FIREPLACE_DI_OFF_DELAY", min = 0, max = 60;
            1184: U16, RW, "USERMODE_REFRESH_DI_OFF_DELAY", min = 0, max = 240;
            1185: U16, RW, "USERMODE_CROWDED_DI_OFF_DELAY", min = 0, max = 8;
            1188: U16, RW, "CDI1_OFF_DELAY", min = 0, max = 240;
            1189: U16, RW, "CDI2_OFF_DELAY", min = 0, max = 240;
            1190: U16, RW, "CDI3_OFF_DELAY", min = 0, max = 240;
            1201: U16, R_, "SPEED_MANUAL_SAF";
            1202: U16, R_, "SPEED_MANUAL_EAF";
            1203: U16, R_, "SPEED_AUTO_SAF";
            1204: U16, R_, "SPEED_AUTO_EAF";
            1205: U16, R_, "SPEED_CROWDED_SAF";
            1206: U16, R_, "SPEED_CROWDED_EAF";
            1207: U16, R_, "SPEED_REFRESH_SAF";
            1208: U16, R_, "SPEED_REFRESH_EAF";
            1209: U16, R_, "SPEED_FIREPLACE_SAF";
            1210: U16, R_, "SPEED_FIREPLACE_EAF";
            1211: U16, R_, "SPEED_AWAY_SAF";
            1212: U16, R_, "SPEED_AWAY_EAF";
            1213: U16, R_, "SPEED_HOLIDAY_SAF";
            1214: U16, R_, "SPEED_HOLIDAY_EAF";
            1215: U16, R_, "SPEED_COOKERHOOD_SAF";
            1216: U16, R_, "SPEED_COOKERHOOD_EAF";
            1217: U16, R_, "SPEED_VACUUMCLEANER_SAF";
            1218: U16, R_, "SPEED_VACUUMCLEANER_EAF";
            1221: U16, R_, "SPEED_CDI1_SAF";
            1222: U16, R_, "SPEED_CDI1_EAF";
            1223: U16, R_, "SPEED_CDI2_SAF";
            1224: U16, R_, "SPEED_CDI2_EAF";
            1225: U16, R_, "SPEED_CDI3_SAF";
            1226: U16, R_, "SPEED_CDI3_EAF";
            1227: U16, R_, "SPEED_PRESSURE_GUARD_SAF";
            1228: U16, R_, "SPEED_PRESSURE_GUARD_EAF";
            1242: U16, R_, "SPEED_INDICATION_APP";
            1251: U16, RW, "FAN_OUTDOOR_COMP_TYPE", min = 0, max = 1;
            1252: U16, RW, "FAN_OUTDOOR_COMP_MAX_VALUE", min = 0, max = 50;
            // Not used since MB FW 1.6.0
            1253: CEL, RW, "FAN_OUTDOOR_COMP_STOP_T_WINTER", min = -300, max = 0;
            1254: CEL, RW, "FAN_OUTDOOR_COMP_MAX_TEMP", min = -300, max = 0;
            1255: U16, R_, "FAN_OUTDOOR_COMP_RESULT", min = 0, max = 100;
            1256: CEL, RW, "FAN_OUTDOOR_COMP_START_T_WINTER", min = -300, max = 0;
            1257: CEL, RW, "FAN_OUTDOOR_COMP_START_T_SUMMER", min = 150, max = 300;
            1258: CEL, RW, "FAN_OUTDOOR_COMP_STOP_T_SUMMER", min = 150, max = 400;
            1259: U16, RW, "FAN_OUTDOOR_COMP_VALUE_SUMMER", min = 0, max = 50;
            1271: U16, RW, "FAN_REGULATION_PBAND", min = 1, max = 3000;
            1272: U16, RW, "FAN_REGULATION_ITIME", min = 0, max = 240;
            1274: U16, RW, "FAN_REGULATION_UNIT", min = 0, max = 4;
            1301: U16, R_, "FAN_LEVEL_SAF_MIN";
            1302: U16, R_, "FAN_LEVEL_EAF_MIN";
            1303: U16, R_, "FAN_LEVEL_SAF_LOW";
            1304: U16, R_, "FAN_LEVEL_EAF_LOW";
            1305: U16, R_, "FAN_LEVEL_SAF_NORMAL";
            1306: U16, R_, "FAN_LEVEL_EAF_NORMAL";
            1307: U16, R_, "FAN_LEVEL_SAF_HIGH";
            1308: U16, R_, "FAN_LEVEL_EAF_HIGH";
            1309: U16, R_, "FAN_LEVEL_SAF_MAX";
            1310: U16, R_, "FAN_LEVEL_EAF_MAX";
            1351: U16, R_, "SPEED_FANS_RUNNING", min = 0, max = 1;
            1352: U16, R_, "SPEED_SAF_DESIRED_OFF", min = 0, max = 1;
            1353: U16, RW, "FAN_MANUAL_STOP_ALLOWED", min = 0, max = 1;
            1357: SEC, R_, "SPEED_ELECTRICAL_HEATER_HOT_COUNTER";
            1358: U16, R_, "FAN_SPEED_AFTER_HEATER_COOLING_DOWN_SAF", min = 0, max = 100;
            1359: U16, R_, "FAN_SPEED_AFTER_HEATER_COOLING_DOWN_EAF", min = 0, max = 100;
            1401: U16, RW, "FAN_LEVEL_SAF_MIN_PERCENTAGE", min = 16, max = 100;
            1402: U16, RW, "FAN_LEVEL_EAF_MIN_PERCENTAGE", min = 16, max = 100;
            1403: U16, RW, "FAN_LEVEL_SAF_LOW_PERCENTAGE", min = 16, max = 100;
            1404: U16, RW, "FAN_LEVEL_EAF_LOW_PERCENTAGE", min = 16, max = 100;
            1405: U16, RW, "FAN_LEVEL_SAF_NORMAL_PERCENTAGE", min = 16, max = 100;
            1406: U16, RW, "FAN_LEVEL_EAF_NORMAL_PERCENTAGE", min = 16, max = 100;
            1407: U16, RW, "FAN_LEVEL_SAF_HIGH_PERCENTAGE", min = 16, max = 100;
            1408: U16, RW, "FAN_LEVEL_EAF_HIGH_PERCENTAGE", min = 16, max = 100;
            1409: U16, RW, "FAN_LEVEL_SAF_MAX_PERCENTAGE", min = 16, max = 100;
            1410: U16, RW, "FAN_LEVEL_EAF_MAX_PERCENTAGE", min = 16, max = 100;
            1411: RPM, RW, "FAN_LEVEL_SAF_MIN_RPM", min = 500, max = 5000;
            1412: RPM, RW, "FAN_LEVEL_EAF_MIN_RPM", min = 500, max = 5000;
            1413: RPM, RW, "FAN_LEVEL_SAF_LOW_RPM", min = 500, max = 5000;
            1414: RPM, RW, "FAN_LEVEL_EAF_LOW_RPM", min = 500, max = 5000;
            1415: RPM, RW, "FAN_LEVEL_SAF_NORMAL_RPM", min = 500, max = 5000;
            1416: RPM, RW, "FAN_LEVEL_EAF_NORMAL_RPM", min = 500, max = 5000;
            1417: RPM, RW, "FAN_LEVEL_SAF_HIGH_RPM", min = 500, max = 5000;
            1418: RPM, RW, "FAN_LEVEL_EAF_HIGH_RPM", min = 500, max = 5000;
            1419: RPM, RW, "FAN_LEVEL_SAF_MAX_RPM", min = 500, max = 5000;
            1420: RPM, RW, "FAN_LEVEL_EAF_MAX_RPM", min = 500, max = 5000;
            1421: U16, RW, "FAN_LEVEL_SAF_MIN_PRESSURE";
            1422: U16, RW, "FAN_LEVEL_EAF_MIN_PRESSURE";
            1423: U16, RW, "FAN_LEVEL_SAF_LOW_PRESSURE";
            1424: U16, RW, "FAN_LEVEL_EAF_LOW_PRESSURE";
            1425: U16, RW, "FAN_LEVEL_SAF_NORMAL_PRESSURE";
            1426: U16, RW, "FAN_LEVEL_EAF_NORMAL_PRESSURE";
            1427: U16, RW, "FAN_LEVEL_SAF_HIGH_PRESSURE";
            1428: U16, RW, "FAN_LEVEL_EAF_HIGH_PRESSURE";
            1429: U16, RW, "FAN_LEVEL_SAF_MAX_PRESSURE";
            1430: U16, RW, "FAN_LEVEL_EAF_MAX_PRESSURE";
            1431: U16, RW, "FAN_LEVEL_SAF_MIN_FLOW";
            1432: U16, RW, "FAN_LEVEL_EAF_MIN_FLOW";
            1433: U16, RW, "FAN_LEVEL_SAF_LOW_FLOW";
            1434: U16, RW, "FAN_LEVEL_EAF_LOW_FLOW";
            1435: U16, RW, "FAN_LEVEL_SAF_NORMAL_FLOW";
            1436: U16, RW, "FAN_LEVEL_EAF_NORMAL_FLOW";
            1437: U16, RW, "FAN_LEVEL_SAF_HIGH_FLOW";
            1438: U16, RW, "FAN_LEVEL_EAF_HIGH_FLOW";
            1439: U16, RW, "FAN_LEVEL_SAF_MAX_FLOW";
            1440: U16, RW, "FAN_LEVEL_EAF_MAX_FLOW";
            1551: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE1_L";
            1552: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE1_H";
            1553: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE2_L";
            1554: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE2_H";
            1555: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE3_L";
            1556: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE3_H";
            1557: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE4_L";
            1558: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE4_H";
            1559: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE5_L";
            1560: U16, R_, "FAN_LOG_SAF_NON_RESETTABLE5_H";
            1561: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE1_L";
            1562: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE1_H";
            1563: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE2_L";
            1564: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE2_H";
            1565: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE3_L";
            1566: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE3_H";
            1567: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE4_L";
            1568: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE4_H";
            1569: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE5_L";
            1570: U16, R_, "FAN_LOG_EAF_NON_RESETTABLE5_H";
            1601: U16, R_, "USERMODE_REMAINING_TIME_AWAY_DI_L";
            1602: U16, R_, "USERMODE_REMAINING_TIME_AWAY_DI_H";
            1603: U16, R_, "USERMODE_REMAINING_TIME_AWAY_HMI_L";
            1604: U16, R_, "USERMODE_REMAINING_TIME_AWAY_HMI_H";
            1605: U16, R_, "USERMODE_REMAINING_TIME_CROWDED_DI_L";
            1606: U16, R_, "USERMODE_REMAINING_TIME_CROWDED_DI_H";
            1607: U16, R_, "USERMODE_REMAINING_TIME_CROWDED_HMI_L";
            1608: U16, R_, "USERMODE_REMAINING_TIME_CROWDED_HMI_H";
            1609: U16, R_, "USERMODE_REMAINING_TIME_HOLIDAY_DI_L";
            1610: U16, R_, "USERMODE_REMAINING_TIME_HOLIDAY_DI_H";
            1611: U16, R_, "USERMODE_REMAINING_TIME_HOLIDAY_HMI_L";
            1612: U16, R_, "USERMODE_REMAINING_TIME_HOLIDAY_HMI_H";
            1613: U16, R_, "USERMODE_REMAINING_TIME_FIREPLACE_DI_L";
            1614: U16, R_, "USERMODE_REMAINING_TIME_FIREPLACE_DI_H";
            1615: U16, R_, "USERMODE_REMAINING_TIME_FIREPLACE_HMI_L";
            1616: U16, R_, "USERMODE_REMAINING_TIME_FIREPLACE_HMI_H";
            1617: U16, R_, "USERMODE_REMAINING_TIME_REFRESH_DI_L";
            1618: U16, R_, "USERMODE_REMAINING_TIME_REFRESH_DI_H";
            1619: U16, R_, "USERMODE_REMAINING_TIME_REFRESH_HMI_L";
            1620: U16, R_, "USERMODE_REMAINING_TIME_REFRESH_HMI_H";
            1621: U16, R_, "USERMODE_REMAINING_TIME_CDI1_L";
            1622: U16, R_, "USERMODE_REMAINING_TIME_CDI1_H";
            1623: U16, R_, "USERMODE_REMAINING_TIME_CDI2_L";
            1624: U16, R_, "USERMODE_REMAINING_TIME_CDI2_H";
            1625: U16, R_, "USERMODE_REMAINING_TIME_CDI3_L";
            1626: U16, R_, "USERMODE_REMAINING_TIME_CDI3_H";
            2001: CEL, RW, "TC_SP", min = 120, max = 300;
            2011: U16, RW, "TC_CASCADE_PBAND", min = 10, max = 600;
            2012: U16, RW, "TC_CASCADE_ITIME", min = 0, max = 240;
            2013: CEL, RW, "TC_CASCADE_SP", min = 120, max = 400;
            2021: CEL, RW, "TC_CASCADE_SP_MIN", min = 120, max = 400;
            2022: CEL, RW, "TC_CASCADE_SP_MAX", min = 120, max = 400;
            2031: U16, RW, "TC_CONTROL_MODE", min = 0, max = 2;
            2041: U16, RW, "TC_CONTROL_PBAND";
            2042: U16, RW, "TC_CONTROL_ITIME";
            2051: CEL, R_, "TC_EAT_RAT_SP", min = 120, max = 400;
            2053: CEL, R_, "TC_ROOM_CTRL_SP_SATC", min = 120, max = 400;
            2054: CEL, R_, "TC_SP_SATC", min = 120, max = 300;
            2055: U16, R_, "SATC_HEAT_DEMAND", min = 0, max = 100;
            2061: CEL, R_, "SATC_PI_SP", min = 120, max = 300;
            2069: I16, R_, "SATC_PI_OUTPUT", min = 0, max = 100;
            2071: CEL, R_, "ROOM_CTRL_PI_SP", min = 120, max = 300;
            2079: I16, R_, "ROOM_CTRL_PI_OUTPUT", min = 0, max = 100;
            2101: PCT, R_, "INPUT_EXTERNAL_CTRL_SAF", min = 0, max = 100;
            2102: PCT, R_, "INPUT_EXTERNAL_CTRL_EAF", min = 0, max = 100;
            2111: DEC, RW, "HEATER_SATC_MIN";
            2112: DEC, RW, "HEATER_SATC_MAX";
            2113: CEL, RW, "HEATER_CIRC_PUMP_START_T", min = 0, max = 200;
            2114: PCT, R_, "HEATER_FROM_SATC", min = 0, max = 100;
            2122: U16, RW, "HEATER_CIRC_PUMP_STOP_DELAY", min = 0, max = 60;
            2124: SEC, R_, "HEATER_CIRC_PUMP_COUNTER", min = 0, max = 3600;
            2131: DEC, RW, "HEAT_EXCHANGER_SATC_MIN";
            2132: DEC, RW, "HEAT_EXCHANGER_SATC_MAX";
            2133: U16, RW, "HEAT_EXCHANGER_TYPE";
            2134: U16, RW, "HEAT_EXCHANGER_COOLING_RECOVERY_ON_OFF", min = 0, max = 1;
            2141: PCT, R_, "HEAT_EXCHANGER_FROM_SATC", min = 0, max = 100;
            2147: U16, R_, "HEAT_EXCHANGER_RH_TRANSFER_CTRL_ENABLED";
            2148: U16, R_, "HEAT_EXCHANGER_SPEED_LIMIT_RH_TRANSFER", min = 0, max = 100;
            2149: U16, R_, "PWM_TRIAC_OUTPUT", min = 0, max = 100;
            2201: U16, RW, "ROTOR_RH_TRANSFER_CTRL_PBAND", min = 0, max = 40;
            2202: U16, RW, "ROTOR_RH_TRANSFER_CTRL_ITIME", min = 0, max = 120;
            2203: U16, RW, "ROTOR_RH_TRANSFER_CTRL_SETPOINT", min = 45, max = 100;
            2204: U16, RW, "ROTOR_RH_TRANSFER_CTRL_ON_OFF", min = 0, max = 1;
            2211: SPH, R_, "ROTOR_EA_SPEC_HUMIDITY", min = 0;
            2212: SPH, R_, "ROTOR_OA_SPEC_HUMIDITY", min = 0;
            2213: SPH, R_, "ROTOR_EA_SPEC_HUMIDITY_SETPOINT", min = 0;
            2221: U16, R_, "ROTOR_CLEANING_DESIRED", min = 0, max = 1;
            2251: U16, RW, "DAMPER_BYPASS_FEEDBACK_SCALING_FACTOR";
            2261: U16, RW, "PASSIVE_HOUSE_CERTIFICATION";
            2262: U16, RW, "PASSIVE_HOUSE_MAX_LIMIT";
            2263: U16, RW, "PASSIVE_HOUSE_ACTIVATION", min = 0, max = 1;
            2301: DEC, RW, "COOLER_SATC_MIN";
            2302: DEC, RW, "COOLER_SATC_MAX";
            2311: U16, R_, "COOLER_FROM_SATC", min = 0, max = 100;
            2314: CEL, RW, "COOLER_CIRC_PUMP_START_T", min = 0, max = 200;
            2315: CEL, RW, "COOLER_RECOVERY_LIMIT_T", min = 0, max = 100;
            2316: CEL, RW, "COOLER_OAT_INTERLOCK_T", min = 120, max = 250;
            2317: U16, RW, "COOLER_CIRC_PUMP_STOP_DELAY", min = 0, max = 60;
            2318: SEC, R_, "COOLER_CIRC_PUMP_COUNTER", min = 0, max = 3600;
            2401: U16, RW, "EXTRA_CONTROLLER_SET_PI_PBAND", min = 10, max = 600;
            2402: SEC, RW, "EXTRA_CONTROLLER_SET_PI_ITIME", min = 0, max = 240;
            2403: CEL, RW, "EXTRA_CONTROLLER_SET_PI_SETPOINT", min = -300, max = 400;
            2404: CEL, RW, "EXTRA_CONTROLLER_CIRC_PUMP_START_T", min = 0, max = 200;
            2405: U16, RW, "EXTRA_CONTROLLER_CIRC_PUMP_STOP_DELAY", min = 0, max = 60;
            2418: U16, RW, "EXTRA_CONTROLLER_PREHEATER_SETPOINT_TYPE", min = 0, max = 1;
            2419: SEC, R_, "EXTRA_CONTROLLER_CIRC_PUMP_COUNTER", min = 0, max = 3600;
            2420: CEL, RW, "EXTRA_CONTROLLER_GEO_PREHEATER_SP", min = -300, max = 100;
            2421: CEL, RW, "EXTRA_CONTROLLER_GEO_PREHEATER_ACTIVATION_T", min = -300, max = 0;
            2422: CEL, RW, "EXTRA_CONTROLLER_GEO_PRECOOLER_SP", min = 100, max = 300;
            2423: CEL, RW, "EXTRA_CONTROLLER_GEO_PRECOOLER_ACTIVATION_T", min = 150, max = 300;
            2424: U16, R_, "EXTRA_CONTROLLER_GEO_SUMMER_WINTER_CNTR_L";
            2425: U16, R_, "EXTRA_CONTROLLER_GEO_SUMMER_WINTER_CNTR_H";
            2426: U16, R_, "EXTRA_CONTROLLER_GEO_SUMMER_WINTER_MODE";
            2427: U16, RW, "EXTRA_CONTROLLER_PREHEATER_DEACTIVATE_AT_HIGH_OAT", min = 0, max = 1;
            2428: CEL, RW, "EXTRA_CONTROLLER_PREHEATER_ACTIVATION_T";
            2451: CEL, RW, "CHANGE_OVER_CIRC_PUMP_START_T", min = 0, max = 200;
            2452: U16, RW, "CHANGE_OVER_CIRC_PUMP_STOP_DELAY", min = 0, max = 60;
            2453: SEC, RW, "CHANGE_OVER_CIRC_PUMP_COUNTER", min = 0, max = 3600;
            2504: CEL, RW, "ECO_T_Y1_OFFSET", min = 0, max = 100;
            2505: U16, RW, "ECO_MODE_ON_OFF", min = 0, max = 1;
            2506: U16, R_, "ECO_FUNCTION_ACTIVE", min = 0, max = 1;
            2521: U16, R_, "ECO_MODE_ACTIVE", min = 0, max = 1;
            3001: U16, RW, "UNIT_CONFIG_SEC_AIR";
            3002: U16, RW, "UNIT_CONFIG_REHEATER_TYPE";
            3003: U16, RW, "UNIT_CONFIG_BYPASS_LOCATION";
            3012: U16, RW, "UNIT_CONFIG_PREHEATER";
            3014: U16, RW, "UNIT_CONFIG_COOLER";
            3015: U16, RW, "UNIT_CONFIG_EXTRA_CONTROLLER";
            3022: U16, RW, "UNIT_CONFIG_RHS_PDM";
            3032: U16, RW, "UNIT_CONFIG_BYF";
            3101: U16, R_, "FUNCTION_ACTIVE_COOLING";
            3102: U16, R_, "FUNCTION_ACTIVE_FREE_COOLING";
            3103: U16, R_, "FUNCTION_ACTIVE_HEATING";
            3104: U16, R_, "FUNCTION_ACTIVE_DEFROSTING";
            3105: U16, R_, "FUNCTION_ACTIVE_HEAT_RECOVERY";
            3106: U16, R_, "FUNCTION_ACTIVE_COOLING_RECOVERY";
            3107: U16, R_, "FUNCTION_ACTIVE_MOISTURE_TRANSFER";
            3108: U16, R_, "FUNCTION_ACTIVE_SECONDARY_AIR";
            3109: U16, R_, "FUNCTION_ACTIVE_VACUUM_CLEANER";
            3110: U16, R_, "FUNCTION_ACTIVE_COOKER_HOOD";
            3111: U16, R_, "FUNCTION_ACTIVE_USER_LOCK";
            3112: U16, R_, "FUNCTION_ACTIVE_ECO_MODE";
            3113: U16, R_, "FUNCTION_ACTIVE_HEATER_COOL_DOWN", min = 0, max = 1;
            3114: U16, R_, "FUNCTION_ACTIVE_PRESSURE_GUARD", min = 0, max = 1;
            3115: U16, R_, "FUNCTION_ACTIVE_CDI_1", min = 0, max = 1;
            3116: U16, R_, "FUNCTION_ACTIVE_CDI_2", min = 0, max = 1;
            3117: U16, R_, "FUNCTION_ACTIVE_CDI_3", min = 0, max = 1;
            4001: U16, RW, "DEFROSTING_MODE", min = 0, max = 2;
            4003: U16, R_, "DEFROSTING_LEVEL", min = 0, max = 5;
            4011: U16, R_, "DEFROSTING_STATE", min = 0, max = 4;
            4012: SEC, R_, "DEFROSTING_COUNTER";
            4014: U16, RW, "DEFROSTING_DISABLE";
            4101: U16, RW, "FREE_COOLING_ON_OFF", min = 0, max = 1;
            4102: CEL, RW, "FREE_COOLING_OUTDOOR_DAYTIME_T", min = 120, max = 300;
            4103: CEL, RW, "FREE_COOLING_OUTDOOR_NIGHTTIME_DEACTIVATION_HIGH_T_LIMIT", min = 70, max = 300;
            4104: CEL, RW, "FREE_COOLING_OUTDOOR_NIGHTTIME_DEACTIVATION_LOW_T_LIMIT", min = 70, max = 300;
            4105: CEL, RW, "FREE_COOLING_ROOM_CANCEL_T", min = 120, max = 300;
            4106: U16, RW, "FREE_COOLING_START_TIME_H";
            4107: U16, RW, "FREE_COOLING_START_TIME_M", min = 0, max = 59;
            4108: U16, RW, "FREE_COOLING_END_TIME_H";
            4109: U16, RW, "FREE_COOLING_END_TIME_M", min = 0, max = 59;
            4111: U16, R_, "FREE_COOLING_ACTIVE", min = 0, max = 1;
            4112: U16, RW, "FREE_COOLING_MIN_SPEED_LEVEL_SAF", min = 3, max = 5;
            4113: U16, RW, "FREE_COOLING_MIN_SPEED_LEVEL_EAF", min = 3, max = 5;
            4114: U16, R_, "FREE_COOLING_STATE", min = 0, max = 3;
            4119: U16, R_, "FREE_COOLING_BLOCK_Y1_COUNTER", min = 0, max = 3600;
            4120: U16, R_, "FREE_COOLING_RELIABLE_TEMPERATURES", min = 0, max = 1;
            5001: CEL, RW, "WS_T_OFFSET_ACTIVE", min = -100, max = 0;
            5002: CEL, RW, "WS_T_OFFSET_INACTIVE", min = -100, max = 0;
            5003: U16, RW, "WS_DAY1_PRD1_START_H", min = 0, max = 23;
            5004: U16, RW, "WS_DAY1_PRD1_START_M", min = 0, max = 59;
            5005: U16, RW, "WS_DAY1_PRD1_END_H", min = 0, max = 23;
            5006: U16, RW, "WS_DAY1_PRD1_END_M", min = 0, max = 59;
            5007: U16, RW, "WS_DAY1_PRD2_START_H", min = 0, max = 23;
            5008: U16, RW, "WS_DAY1_PRD2_START_M", min = 0, max = 59;
            5009: U16, RW, "WS_DAY1_PRD2_END_H", min = 0, max = 23;
            5010: U16, RW, "WS_DAY1_PRD2_END_M", min = 0, max = 59;
            5011: U16, RW, "WS_DAY2_PRD1_START_H", min = 0, max = 23;
            5012: U16, RW, "WS_DAY2_PRD1_START_M", min = 0, max = 59;
            5013: U16, RW, "WS_DAY2_PRD1_END_H", min = 0, max = 23;
            5014: U16, RW, "WS_DAY2_PRD1_END_M", min = 0, max = 59;
            5015: U16, RW, "WS_DAY2_PRD2_START_H", min = 0, max = 23;
            5016: U16, RW, "WS_DAY2_PRD2_START_M", min = 0, max = 59;
            5017: U16, RW, "WS_DAY2_PRD2_END_H", min = 0, max = 23;
            5018: U16, RW, "WS_DAY2_PRD2_END_M", min = 0, max = 59;
            5019: U16, RW, "WS_DAY3_PRD1_START_H", min = 0, max = 23;
            5020: U16, RW, "WS_DAY3_PRD1_START_M", min = 0, max = 59;
            5021: U16, RW, "WS_DAY3_PRD1_END_H", min = 0, max = 23;
            5022: U16, RW, "WS_DAY3_PRD1_END_M", min = 0, max = 59;
            5023: U16, RW, "WS_DAY3_PRD2_START_H", min = 0, max = 23;
            5024: U16, RW, "WS_DAY3_PRD2_START_M", min = 0, max = 59;
            5025: U16, RW, "WS_DAY3_PRD2_END_H", min = 0, max = 23;
            5026: U16, RW, "WS_DAY3_PRD2_END_M", min = 0, max = 59;
            5027: U16, RW, "WS_DAY4_PRD1_START_H", min = 0, max = 23;
            5028: U16, RW, "WS_DAY4_PRD1_START_M", min = 0, max = 59;
            5029: U16, RW, "WS_DAY4_PRD1_END_H", min = 0, max = 23;
            5030: U16, RW, "WS_DAY4_PRD1_END_M", min = 0, max = 59;
            5031: U16, RW, "WS_DAY4_PRD2_START_H", min = 0, max = 23;
            5032: U16, RW, "WS_DAY4_PRD2_START_M", min = 0, max = 59;
            5033: U16, RW, "WS_DAY4_PRD2_END_H", min = 0, max = 23;
            5034: U16, RW, "WS_DAY4_PRD2_END_M", min = 0, max = 59;
            5035: U16, RW, "WS_DAY5_PRD1_START_H", min = 0, max = 23;
            5036: U16, RW, "WS_DAY5_PRD1_START_M", min = 0, max = 59;
            5037: U16, RW, "WS_DAY5_PRD1_END_H", min = 0, max = 23;
            5038: U16, RW, "WS_DAY5_PRD1_END_M", min = 0, max = 59;
            5039: U16, RW, "WS_DAY5_PRD2_START_H", min = 0, max = 23;
            5040: U16, RW, "WS_DAY5_PRD2_START_M", min = 0, max = 59;
            5041: U16, RW, "WS_DAY5_PRD2_END_H", min = 0, max = 23;
            5042: U16, RW, "WS_DAY5_PRD2_END_M", min = 0, max = 59;
            5043: U16, RW, "WS_DAY6_PRD1_START_H", min = 0, max = 23;
            5044: U16, RW, "WS_DAY6_PRD1_START_M", min = 0, max = 59;
            5045: U16, RW, "WS_DAY6_PRD1_END_H", min = 0, max = 23;
            5046: U16, RW, "WS_DAY6_PRD1_END_M", min = 0, max = 59;
            5047: U16, RW, "WS_DAY6_PRD2_START_H", min = 0, max = 23;
            5048: U16, RW, "WS_DAY6_PRD2_START_M", min = 0, max = 59;
            5049: U16, RW, "WS_DAY6_PRD2_END_H", min = 0, max = 23;
            5050: U16, RW, "WS_DAY6_PRD2_END_M", min = 0, max = 59;
            5051: U16, RW, "WS_DAY7_PRD1_START_H", min = 0, max = 23;
            5052: U16, RW, "WS_DAY7_PRD1_START_M", min = 0, max = 59;
            5053: U16, RW, "WS_DAY7_PRD1_END_H", min = 0, max = 23;
            5054: U16, RW, "WS_DAY7_PRD1_END_M", min = 0, max = 59;
            5055: U16, RW, "WS_DAY7_PRD2_START_H", min = 0, max = 23;
            5056: U16, RW, "WS_DAY7_PRD2_START_M", min = 0, max = 59;
            5057: U16, RW, "WS_DAY7_PRD2_END_H", min = 0, max = 23;
            5058: U16, RW, "WS_DAY7_PRD2_END_M", min = 0, max = 59;
            5059: U16, R_, "WS_ACTIVE", min = 0, max = 1;
            5060: U16, RW, "WS_FAN_LEVEL_SCHEDULED", min = 1, max = 5;
            5061: U16, RW, "WS_FAN_LEVEL_UNSCHEDULED", min = 1, max = 5;
            5062: U16, R_, "WS_ANY_DEFINED";
            5101: U16, RW, "WS_DAY1_PRD1_ENABLED", min = 0, max = 1;
            5102: U16, RW, "WS_DAY1_PRD2_ENABLED", min = 0, max = 1;
            5103: U16, RW, "WS_DAY2_PRD1_ENABLED", min = 0, max = 1;
            5104: U16, RW, "WS_DAY2_PRD2_ENABLED", min = 0, max = 1;
            5105: U16, RW, "WS_DAY3_PRD1_ENABLED", min = 0, max = 1;
            5106: U16, RW, "WS_DAY3_PRD2_ENABLED", min = 0, max = 1;
            5107: U16, RW, "WS_DAY4_PRD1_ENABLED", min = 0, max = 1;
            5108: U16, RW, "WS_DAY4_PRD2_ENABLED", min = 0, max = 1;
            5109: U16, RW, "WS_DAY5_PRD1_ENABLED", min = 0, max = 1;
            5110: U16, RW, "WS_DAY5_PRD2_ENABLED", min = 0, max = 1;
            5111: U16, RW, "WS_DAY6_PRD1_ENABLED", min = 0, max = 1;
            5112: U16, RW, "WS_DAY6_PRD2_ENABLED", min = 0, max = 1;
            5113: U16, RW, "WS_DAY7_PRD1_ENABLED", min = 0, max = 1;
            5114: U16, RW, "WS_DAY7_PRD2_ENABLED", min = 0, max = 1;
            6001: U16, RW, "TIME_YEAR", min = 0, max = 2999;
            6002: U16, RW, "TIME_MONTH", min = 1, max = 12;
            6003: U16, RW, "TIME_DAY", min = 1, max = 31;
            6004: U16, RW, "TIME_HOUR", min = 0, max = 23;
            6005: U16, RW, "TIME_MINUTE", min = 0, max = 59;
            6006: U16, RW, "TIME_SECOND", min = 0, max = 59;
            6007: U16, RW, "TIME_AUTO_SUM_WIN", min = 0, max = 1;
            6008: U16, RW, "HOUR_FORMAT", min = 0, max = 1;
            6009: U16, R_, "DAY_OF_THE_WEEK", min = 0, max = 6;
            6010: U16, R_, "DST_PERIOD_ACTIVE", min = 0, max = 1;
            6011: U16, R_, "TIME_RTC_SECONDS_L";
            6012: U16, R_, "TIME_RTC_SECONDS_H";
            6021: U16, R_, "SYSTEM_START_UP_TIME_L";
            6022: U16, R_, "SYSTEM_START_UP_TIME_H";
            6101: U16, R_, "TIME_RTC";
            7001: U16, RW, "FILTER_PERIOD", min = 3, max = 15;
            7002: U16, RW, "FILTER_REPLACEMENT_TIME_L";
            7003: U16, RW, "FILTER_REPLACEMENT_TIME_H";
            7004: U16, R_, "FILTER_PERIOD_SET";
            7005: U16, R_, "FILTER_REMAINING_TIME_L";
            7006: U16, R_, "FILTER_REMAINING_TIME_H";
            7007: U16, R_, "FILTER_ALARM_WAS_DETECTED";
            7008: U16, RW, "FILTER_KIT_INDEX";
            8004: U16, R_, "SYSTEM_COUNTRY";
            8021: U16, R_, "SYSTEM_UNIT_MODEL1";
            8061: U16, R_, "SYSTEM_ITEM_NUMBER1";
            8201: U16, R_, "SYSTEM_COMPANY1";
            8401: U16, R_, "SYSTEM_TELEPHONE1";
            8601: U16, R_, "SYSTEM_HOMEPAGE1";
            8801: U16, R_, "SYSTEM_EMAIL1";
            8901: U16, R_, "SYSTEM_SERIAL_NUMBER1";
            8951: U16, R_, "SYSTEM_UNIT_MODEL_TYPE";
            9001: U16, RW, "SYSTEM_UNIT_FLOW", min = 0, max = 2;
            9002: U16, RW, "SYSTEM_UNIT_PRESSURE", min = 0, max = 1;
            9003: U16, RW, "SYSTEM_UNIT_TEMPERATURE", min = 0, max = 1;
            11001: U16, RW, "AI_1_FUNCTION", min = 0, max = 8;
            11002: U16, RW, "AI_2_FUNCTION", min = 0, max = 8;
            11003: U16, RW, "AI_3_FUNCTION", min = 0, max = 8;
            11004: U16, RW, "AI_4_FUNCTION", min = 0, max = 8;
            11005: U16, RW, "AI_5_FUNCTION", min = 0, max = 8;
            11006: U16, RW, "AI_6_FUNCTION", min = 0, max = 8;
            11007: U16, RW, "AI_7_FUNCTION", min = 0, max = 8;
            11021: CEL, RW, "AI_1_CORRECTION";
            11022: CEL, RW, "AI_2_CORRECTION";
            11023: CEL, RW, "AI_3_CORRECTION";
            11024: CEL, RW, "AI_4_CORRECTION";
            11025: CEL, RW, "AI_5_CORRECTION";
            11026: CEL, RW, "AI_6_CORRECTION";
            11027: CEL, RW, "AI_7_CORRECTION";
            11101: U16, RW, "UI_1_MODE", min = 0, max = 2;
            11102: U16, RW, "UI_2_MODE", min = 0, max = 2;
            11103: U16, RW, "UI_3_MODE", min = 0, max = 2;
            11104: U16, RW, "UI_4_MODE", min = 0, max = 2;
            11105: U16, RW, "UI_5_MODE", min = 0, max = 2;
            11106: U16, RW, "UI_6_MODE", min = 0, max = 2;
            11201: U16, RW, "UI_1_FUNCTION", min = 0, max = 5;
            11202: U16, RW, "UI_2_FUNCTION", min = 0, max = 5;
            11203: U16, RW, "UI_3_FUNCTION", min = 0, max = 5;
            11204: U16, RW, "UI_4_FUNCTION", min = 0, max = 5;
            11205: U16, RW, "UI_5_FUNCTION", min = 0, max = 5;
            11206: U16, RW, "UI_6_FUNCTION", min = 0, max = 5;
            11221: I16, RW, "UI_CFG_CORRECTION_ANALOG1";
            11222: I16, RW, "UI_CFG_CORRECTION_ANALOG2";
            11223: I16, RW, "UI_CFG_CORRECTION_ANALOG3";
            11224: I16, RW, "UI_CFG_CORRECTION_ANALOG4";
            11225: I16, RW, "UI_CFG_CORRECTION_ANALOG5";
            11226: I16, RW, "UI_CFG_CORRECTION_ANALOG6";
            11301: U16, RW, "UI_CONNECTION_DIGITAL1", min = 0, max = 18;
            11302: U16, RW, "UI_CONNECTION_DIGITAL2", min = 0, max = 18;
            11303: U16, RW, "UI_CONNECTION_DIGITAL3", min = 0, max = 18;
            11304: U16, RW, "UI_CONNECTION_DIGITAL4", min = 0, max = 18;
            11305: U16, RW, "UI_CONNECTION_DIGITAL5", min = 0, max = 18;
            11306: U16, RW, "UI_CONNECTION_DIGITAL6", min = 0, max = 18;
            11321: U16, RW, "UI_CFG_POLARITY_DIGITAL1", min = 0, max = 1;
            11322: U16, RW, "UI_CFG_POLARITY_DIGITAL2", min = 0, max = 1;
            11323: U16, RW, "UI_CFG_POLARITY_DIGITAL3", min = 0, max = 1;
            11324: U16, RW, "UI_CFG_POLARITY_DIGITAL4", min = 0, max = 1;
            11325: U16, RW, "UI_CFG_POLARITY_DIGITAL5", min = 0, max = 1;
            11326: U16, RW, "UI_CFG_POLARITY_DIGITAL6", min = 0, max = 1;
            11401: U16, RW, "DI_CONNECTION_1", min = 0, max = 18;
            11402: U16, RW, "DI_CONNECTION_2", min = 0, max = 18;
            11411: U16, RW, "FAN_FEEDBACK_TYPE";
            11421: U16, RW, "DI_CFG_POLARITY_1", min = 0, max = 1;
            11422: U16, RW, "DI_CFG_POLARITY_2", min = 0, max = 1;
            11501: U16, RW, "PDM_CONNECTED_RH";
            11502: U16, RW, "PDM_CONNECTED_T";
            11503: I16, RW, "PDM_CORRECTION_RH";
            11504: CEL, RW, "PDM_CORRECTION_T";
            12011: MV, R_, "INPUT_ANALOG_UI_1";
            12012: MV, R_, "INPUT_ANALOG_UI_2";
            12013: MV, R_, "INPUT_ANALOG_UI_3";
            12014: MV, R_, "INPUT_ANALOG_UI_4";
            12015: MV, R_, "INPUT_ANALOG_UI_5";
            12016: MV, R_, "INPUT_ANALOG_UI_6";
            12021: U16, R_, "INPUT_DIGITAL_UI_1", min = 0, max = 1;
            12022: U16, R_, "INPUT_DIGITAL_UI_2", min = 0, max = 1;
            12023: U16, R_, "INPUT_DIGITAL_UI_3", min = 0, max = 1;
            12024: U16, R_, "INPUT_DIGITAL_UI_4", min = 0, max = 1;
            12025: U16, R_, "INPUT_DIGITAL_UI_5", min = 0, max = 1;
            12026: U16, R_, "INPUT_DIGITAL_UI_6";
            12031: U16, R_, "INPUT_DIGITAL_DI_1", min = 0, max = 1;
            12032: U16, R_, "INPUT_DIGITAL_DI_2", min = 0, max = 1;
            12101: CEL, RW, "SENSOR_FPT", min = -400, max = 800;
            12102: CEL, RW, "SENSOR_OAT", min = -400, max = 800;
            12103: CEL, RW, "SENSOR_SAT", min = -400, max = 800;
            12104: CEL, RW, "SENSOR_RAT", min = -400, max = 800;
            12105: CEL, RW, "SENSOR_EAT", min = -400, max = 800;
            12106: CEL, RW, "SENSOR_ECT", min = -400, max = 800;
            12107: CEL, RW, "SENSOR_EFT", min = -400, max = 800;
            12108: CEL, RW, "SENSOR_OHT", min = -400, max = 800;
            12109: U16, RW, "SENSOR_RHS", min = 0, max = 100;
            12110: U16, R_, "SENSOR_BYS", min = 0, max = 1;
            12111: U16, R_, "SENSOR_EMT", min = 0, max = 1;
            12112: U16, R_, "SENSOR_RGS", min = 0, max = 1;
            12113: PPM, RW, "SENSOR_MODBUS_CO2", min = 0, max = 2000;
            12114: U16, RW, "SENSOR_MODBUS_RH", min = 0, max = 100;
            12115: PPM, RW, "SENSOR_CO2S", min = 0, max = 2000;
            12136: U16, RW, "SENSOR_RHS_PDM", min = 0, max = 100;
            12151: PPM, RW, "SENSOR_CO2S_1", min = 0, max = 2000;
            12152: PPM, RW, "SENSOR_CO2S_2", min = 0, max = 2000;
            12153: PPM, RW, "SENSOR_CO2S_3", min = 0, max = 2000;
            12154: PPM, RW, "SENSOR_CO2S_4", min = 0, max = 2000;
            12155: PPM, RW, "SENSOR_CO2S_5", min = 0, max = 2000;
            12156: PPM, R_, "SENSOR_CO2S_6";
            12161: U16, RW, "SENSOR_RHS_1", min = 0, max = 100;
            12162: U16, RW, "SENSOR_RHS_2", min = 0, max = 100;
            12163: U16, RW, "SENSOR_RHS_3", min = 0, max = 100;
            12164: U16, RW, "SENSOR_RHS_4", min = 0, max = 100;
            12165: U16, RW, "SENSOR_RHS_5", min = 0, max = 100;
            12166: U16, R_, "SENSOR_RHS_6", min = 0, max = 100;
            12201: U16, R_, "SENSOR_P_SAF";
            12202: U16, R_, "SENSOR_P_EAF";
            12203: U16, R_, "SENSOR_FLOW_SAF";
            12204: U16, R_, "SENSOR_FLOW_EAF";
            12301: U16, R_, "SENSOR_DI_AWAY", min = 0, max = 1;
            12302: U16, R_, "SENSOR_DI_HOLIDAY", min = 0, max = 1;
            12303: U16, R_, "SENSOR_DI_FIREPLACE", min = 0, max = 1;
            12304: U16, R_, "SENSOR_DI_REHRESH", min = 0, max = 1;
            12305: U16, R_, "SENSOR_DI_CROWDED", min = 0, max = 1;
            12306: U16, R_, "SENSOR_DI_COOKERHOOD", min = 0, max = 1;
            12307: U16, R_, "SENSOR_DI_VACUUMCLEANER", min = 0, max = 1;
            12308: U16, R_, "SENSOR_DI_EXTERNAL_STOP", min = 0, max = 1;
            12309: U16, R_, "SENSOR_DI_LOAD_DETECTED", min = 0, max = 1;
            12310: U16, R_, "SENSOR_DI_EXTRA_CONTROLLER_EMT", min = 0, max = 1;
            12311: U16, R_, "SENSOR_DI_FIRE_ALARM", min = 0, max = 1;
            12312: U16, R_, "SENSOR_DI_CHANGE_OVER_FEEDBACK", min = 0, max = 1;
            12316: U16, R_, "SENSOR_DI_PRESSURE_GUARD", min = 0, max = 1;
            12317: U16, R_, "SENSOR_DI_CDI_1", min = 0, max = 1;
            12318: U16, R_, "SENSOR_DI_CDI_2", min = 0, max = 1;
            12319: U16, R_, "SENSOR_DI_CDI_3", min = 0, max = 1;
            12401: RPM, R_, "SENSOR_RPM_SAF", min = 0, max = 5000;
            12402: RPM, R_, "SENSOR_RPM_EAF", min = 0, max = 5000;
            12403: U16, R_, "SENSOR_FLOW_PIGGYBACK_SAF";
            12404: U16, R_, "SENSOR_FLOW_PIGGYBACK_EAF";
            12405: PCT, R_, "SENSOR_DI_BYF";
            12543: U16, R_, "SENSOR_PDM_EAT_CONFIGURED";
            12544: CEL, RW, "SENSOR_PDM_EAT_VALUE", min = -400, max = 800;
            12602: U16, RW, "ALARM_FLOW_MIN_SAF";
            12603: U16, RW, "ALARM_PRESSURE_SENSOR_MIN_SAF";
            12604: U16, RW, "ALARM_FLOW_MIN_EAF";
            12605: U16, RW, "ALARM_PRESSURE_SENSOR_MIN_EAF";
            12701: U16, RW, "SENSOR_PRESSURE_AT_0V_SAF";
            12702: U16, RW, "SENSOR_PRESSURE_AT_10V_SAF";
            12704: U16, RW, "SENSOR_PRESSURE_ALARM_SAF";
            12706: U16, RW, "SENSOR_PRESSURE_FILTER_FACTOR_DYNAMICAL_A_COEF_SAF";
            12707: U16, RW, "SENSOR_PRESSURE_FILTER_FACTOR_DYNAMICAL_B_COEF_SAF";
            12801: U16, RW, "SENSOR_PRESSURE_AT_0V_EAF";
            12802: U16, RW, "SENSOR_PRESSURE_AT_10V_EAF";
            12804: U16, RW, "SENSOR_PRESSURE_ALARM_EAF";
            12806: U16, RW, "SENSOR_PRESSURE_FILTER_FACTOR_DYNAMICAL_A_COEF_EAF";
            12807: U16, RW, "SENSOR_PRESSURE_FILTER_FACTOR_DYNAMICAL_B_COEF_EAF";
            12929: U16, RW, "MANUAL_OVERRIDE_F_INPUT_UI_SAFC_MODE", min = 0, max = 1;
            12930: U16, RW, "MANUAL_OVERRIDE_F_INPUT_UI_EAFC_MODE", min = 0, max = 1;
            12931: U16, RW, "MANUAL_OVERRIDE_F_INPUT_UI_RH_MODE", min = 0, max = 1;
            12932: U16, RW, "MANUAL_OVERRIDE_F_INPUT_UI_CO2_MODE", min = 0, max = 1;
            12933: U16, RW, "MANUAL_OVERRIDE_F_INPUT_OAT_MODE", min = 0, max = 1;
            12934: U16, RW, "MANUAL_OVERRIDE_F_INPUT_SAT_MODE", min = 0, max = 1;
            12935: U16, RW, "MANUAL_OVERRIDE_F_INPUT_OHT_MODE", min = 0, max = 1;
            12936: U16, RW, "MANUAL_OVERRIDE_F_INPUT_FPT_MODE", min = 0, max = 1;
            12937: U16, RW, "MANUAL_OVERRIDE_F_INPUT_RAT_MODE", min = 0, max = 1;
            12938: U16, RW, "MANUAL_OVERRIDE_F_INPUT_EAT_MODE", min = 0, max = 1;
            12939: U16, RW, "MANUAL_OVERRIDE_F_INPUT_ECT_MODE", min = 0, max = 1;
            12940: U16, RW, "MANUAL_OVERRIDE_F_INPUT_EFT_MODE", min = 0, max = 1;
            12941: U16, RW, "MANUAL_OVERRIDE_F_INPUT_PDM_RH_MODE", min = 0, max = 1;
            12942: U16, RW, "MANUAL_OVERRIDE_F_INPUT_PDM_T_MODE", min = 0, max = 1;
            12943: U16, RW, "MANUAL_OVERRIDE_INPUT_SAF_RPM_MODE", min = 0, max = 1;
            12944: U16, RW, "MANUAL_OVERRIDE_INPUT_EAF_RPM_MODE", min = 0, max = 1;
            12945: U16, RW, "MANUAL_OVERRIDE_INPUT_UI6_MODE", min = 0, max = 1;
            12946: U16, RW, "MANUAL_OVERRIDE_INPUT_BYF_MODE", min = 0, max = 1;
            12947: U16, RW, "MANUAL_OVERRIDE_INPUT_PIGGYBACK1_SAF_P_MODE", min = 0, max = 1;
            12948: U16, RW, "MANUAL_OVERRIDE_INPUT_PIGGYBACK1_EAF_P_MODE", min = 0, max = 1;
            12949: U16, RW, "MANUAL_OVERRIDE_INPUT_PIGGYBACK2_SAF_P_MODE", min = 0, max = 1;
            12950: U16, RW, "MANUAL_OVERRIDE_INPUT_PIGGYBACK2_EAF_P_MODE", min = 0, max = 1;
            12951: I16, RW, "MANUAL_OVERRIDE_INPUT_AI1_VALUE", min = -410, max = 810;
            12952: I16, RW, "MANUAL_OVERRIDE_INPUT_AI2_VALUE", min = -410, max = 810;
            12953: I16, RW, "MANUAL_OVERRIDE_INPUT_AI3_VALUE", min = -410, max = 810;
            12954: I16, RW, "MANUAL_OVERRIDE_INPUT_AI4_VALUE", min = -410, max = 810;
            12955: I16, RW, "MANUAL_OVERRIDE_INPUT_AI5_VALUE", min = -410, max = 810;
            12956: I16, RW, "MANUAL_OVERRIDE_INPUT_AI6_VALUE", min = -410, max = 810;
            12957: I16, RW, "MANUAL_OVERRIDE_INPUT_AI7_VALUE", min = -410, max = 810;
            12958: I16, RW, "MANUAL_OVERRIDE_INPUT_DI1_VALUE", min = 0, max = 1;
            12959: I16, RW, "MANUAL_OVERRIDE_INPUT_DI2_VALUE", min = 0, max = 1;
            12960: I16, RW, "MANUAL_OVERRIDE_INPUT_UI1_VALUE", min = 0, max = 100;
            12961: I16, RW, "MANUAL_OVERRIDE_INPUT_UI2_VALUE", min = 0, max = 100;
            12962: I16, RW, "MANUAL_OVERRIDE_INPUT_UI3_VALUE", min = 0, max = 100;
            12963: I16, RW, "MANUAL_OVERRIDE_INPUT_UI4_VALUE", min = 0, max = 100;
            12964: I16, RW, "MANUAL_OVERRIDE_INPUT_UI5_VALUE", min = 0, max = 100;
            12979: I16, RW, "MANUAL_OVERRIDE_F_INPUT_SAFC_VALUE", min = 0, max = 100;
            12980: I16, RW, "MANUAL_OVERRIDE_F_INPUT_EAFC_VALUE", min = 0, max = 100;
            12983: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_OAT_VALUE", min = -410, max = 810;
            12984: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_SAT_VALUE", min = -410, max = 810;
            12985: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_OHT_VALUE", min = -410, max = 810;
            12986: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_FPT_VALUE", min = -410, max = 810;
            12987: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_RAT_VALUE", min = -410, max = 810;
            12988: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_EAT_VALUE", min = -410, max = 810;
            12989: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_ECT_VALUE", min = -410, max = 810;
            12990: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_EFT_VALUE", min = -410, max = 810;
            13001: U16, RW, "AO_CFG_CONNECTION_1";
            13002: U16, RW, "AO_CFG_CONNECTION_2";
            13003: U16, RW, "AO_CFG_CONNECTION_3";
            13004: U16, RW, "AO_CFG_CONNECTION_4";
            13005: U16, RW, "AO_CFG_CONNECTION_5";
            13021: U16, RW, "CFG_HEATER_ACTUATOR_TYPE";
            13022: U16, RW, "CFG_HEAT_EXCHANGER_ACTUATOR_TYPE";
            13023: U16, RW, "CFG_COOLER_ACTUATOR_TYPE";
            13024: U16, RW, "CFG_EXTRA_ACTUATOR_TYPE";
            13025: U16, RW, "CFG_CHANGE_OVER_ACTUATOR_TYPE";
            13026: U16, RW, "CFG_AO_T_SP_ACTUATOR_TYPE";
            13031: CEL, RW, "CFG_AO_T_SP_LOW_VALUE";
            13032: CEL, RW, "CFG_AO_T_SP_HIGH_VALUE";
            13033: U16, RW, "CFG_AO_T_SP_TYPE";
            13034: DV, R_, "AO_T_SP_VALUE";
            13101: U16, RW, "DO_CFG_CONNECTION_1";
            13102: U16, RW, "DO_CFG_CONNECTION_2";
            13103: U16, RW, "DO_CFG_CONNECTION_3";
            13104: U16, RW, "DO_CFG_CONNECTION_4";
            13201: U16, RW, "OUTPUT_TRIAC_CONFIGURED", min = 0, max = 1;
            13301: U16, R_, "DO1_AFTER_MUX", min = 0, max = 1;
            13302: U16, R_, "DO2_AFTER_MUX", min = 0, max = 1;
            13303: U16, R_, "DO3_AFTER_MUX", min = 0, max = 1;
            13304: U16, R_, "DO4_AFTER_MUX", min = 0, max = 1;
            13311: U16, R_, "AO1_AFTER_MUX", min = 0, max = 100;
            13312: U16, R_, "AO2_AFTER_MUX", min = 0, max = 100;
            13313: U16, R_, "AO3_AFTER_MUX", min = 0, max = 100;
            13314: U16, R_, "AO4_AFTER_MUX", min = 0, max = 100;
            13315: U16, R_, "AO5_AFTER_MUX", min = 0, max = 100;
            13501: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO1";
            13502: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO2";
            13503: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO3";
            13504: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO4";
            13505: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO5";
            13506: U16, RW, "MANUAL_OVERRIDE_OUTPUT_DO1";
            13507: U16, RW, "MANUAL_OVERRIDE_OUTPUT_DO2";
            13508: U16, RW, "MANUAL_OVERRIDE_OUTPUT_DO3";
            13509: U16, RW, "MANUAL_OVERRIDE_OUTPUT_DO4";
            13510: U16, RW, "MANUAL_OVERRIDE_OUTPUT_TRIAC";
            13601: U16, RW, "MANUAL_OVERRIDE_OUTPUT_SAF", min = 0, max = 1;
            13602: U16, RW, "MANUAL_OVERRIDE_OUTPUT_EAF", min = 0, max = 1;
            13701: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO1_VALUE";
            13702: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO2_VALUE";
            13703: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO3_VALUE";
            13704: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO4_VALUE";
            13705: U16, RW, "MANUAL_OVERRIDE_OUTPUT_AO5_VALUE";
            13706: U16, RW, "MANUAL_OVERRIDE_OUTPUT_DO1_VALUE";
            13707: U16, RW, "MANUAL_OVERRIDE_OUTPUT_DO2_VALUE";
            13708: U16, RW, "MANUAL_OVERRIDE_OUTPUT_DO3_VALUE";
            13709: U16, RW, "MANUAL_OVERRIDE_OUTPUT_DO4_VALUE";
            13710: U16, RW, "MANUAL_OVERRIDE_OUTPUT_TRIAC_VALUE";
            13801: U16, RW, "MANUAL_OVERRIDE_OUTPUT_SAF_VALUE", min = 0, max = 100;
            13802: U16, RW, "MANUAL_OVERRIDE_OUTPUT_EAF_VALUE", min = 0, max = 100;
            14001: U16, R_, "OUTPUT_SAF", min = 0, max = 100;
            14002: U16, R_, "OUTPUT_EAF", min = 0, max = 100;
            14003: U16, R_, "OUTPUT_ALARM", min = 0, max = 1;
            14004: U16, R_, "OUTPUT_OUTDOOR_EXTRACT_DAMPER", min = 0, max = 1;
            14101: U16, R_, "OUTPUT_Y1_ANALOG", min = 0, max = 100;
            14102: U16, R_, "OUTPUT_Y1_DIGITAL", min = 0, max = 1;
            14103: U16, R_, "OUTPUT_Y2_ANALOG", min = 0, max = 100;
            14104: U16, R_, "OUTPUT_Y2_DIGITAL", min = 0, max = 1;
            14201: U16, R_, "OUTPUT_Y3_ANALOG", min = 0, max = 100;
            14202: U16, R_, "OUTPUT_Y3_DIGITAL", min = 0, max = 1;
            14203: U16, R_, "OUTPUT_Y4_ANALOG", min = 0, max = 100;
            14204: U16, R_, "OUTPUT_Y4_DIGITAL", min = 0, max = 1;
            14301: U16, R_, "OUTPUT_Y1_CIRC_PUMP";
            14302: U16, R_, "OUTPUT_Y3_CIRC_PUMP";
            14303: U16, R_, "OUTPUT_Y1_Y3_CIRC_PUMP";
            14304: U16, R_, "OUTPUT_Y4_CIRC_PUMP";
            14305: U16, R_, "OUTPUT_Y3_ACTIVATE_COIL", min = 0, max = 1;
            14306: PCT, R_, "OUTPUT_Y1_Y3_ANALOG", min = 0, max = 100;
            14307: U16, R_, "OUTPUT_Y4_SECONDARY_AIR", min = 0, max = 1;
            14308: U16, R_, "INTERLOCK_EXTERNAL_FAN_CONTROL", min = 0, max = 1;
            14309: U16, R_, "OUTPUT_UNIT_STATUS_OK", min = 0, max = 1;
            14310: U16, R_, "OUTPUT_WS_RUNNING_UNSCHEDULED", min = 0, max = 1;
            14311: U16, R_, "OUTPUT_WS_RUNNING_SCHEDULED", min = 0, max = 1;
            14351: U16, R_, "OUTPUT_AO1";
            14352: U16, R_, "OUTPUT_AO2";
            14353: U16, R_, "OUTPUT_AO3";
            14354: U16, R_, "OUTPUT_AO4";
            14355: U16, R_, "OUTPUT_AO5";
            14361: U16, R_, "OUTPUT_DO1", min = 0, max = 1;
            14362: U16, R_, "OUTPUT_DO2", min = 0, max = 1;
            14363: U16, R_, "OUTPUT_DO3", min = 0, max = 1;
            14364: U16, R_, "OUTPUT_DO4", min = 0, max = 1;
            14371: PCT, R_, "OUTPUT_FAN_SPEED1", min = 0, max = 100;
            14372: PCT, R_, "OUTPUT_FAN_SPEED2", min = 0, max = 100;
            14381: U16, R_, "OUTPUT_TRIAC", min = 0, max = 1;
            14401: U16, RW, "K_FACTOR_SAF", min = 0, max = 1000;
            14402: U16, RW, "K_FACTOR_EAF", min = 0, max = 1000;
            15001: U16, R_, "ALARM_SAF_CTRL_ERROR", min = 0, max = 1;
            15002: U16, R_, "ALARM_SAF_CTRL_ALARM", min = 0, max = 3;
            15003: U16, RW, "ALARM_SAF_CTRL_CLEAR_ALARM", min = 0, max = 1;
            15007: SEC, R_, "ALARM_SAF_CTRL_ERROR_DURATION_COUNTER";
            15008: U16, R_, "ALARM_EAF_CTRL_ERROR", min = 0, max = 1;
            15009: U16, R_, "ALARM_EAF_CTRL_ALARM", min = 0, max = 3;
            15010: U16, RW, "ALARM_EAF_CTRL_CLEAR_ALARM", min = 0, max = 1;
            15014: SEC, R_, "ALARM_EAF_CTRL_ERROR_DURATION_COUNTER";
            15015: U16, R_, "ALARM_FROST_PROT_ERROR", min = 0, max = 1;
            15016: U16, R_, "ALARM_FROST_PROT_ALARM", min = 0, max = 3;
            15017: U16, RW, "ALARM_FROST_PROT_CLEAR_ALARM", min = 0, max = 1;
            15021: SEC, R_, "ALARM_FROST_PROT_ERROR_DURATION_COUNTER";
            15022: U16, R_, "ALARM_DEFROSTING_ERROR", min = 0, max = 1;
            15023: U16, R_, "ALARM_DEFROSTING_ALARM", min = 0, max = 3;
            15024: U16, RW, "ALARM_DEFROSTING_CLEAR_ALARM", min = 0, max = 1;
            15028: SEC, R_, "ALARM_DEFROSTING_ERROR_DURATION_COUNTER";
            15029: U16, R_, "ALARM_SAF_RPM_ERROR", min = 0, max = 1;
            15030: U16, R_, "ALARM_SAF_RPM_ALARM", min = 0, max = 3;
            15031: U16, RW, "ALARM_SAF_RPM_CLEAR_ALARM", min = 0, max = 1;
            15035: SEC, R_, "ALARM_SAF_RPM_ERROR_DURATION_COUNTER";
            15036: U16, R_, "ALARM_EAF_RPM_ERROR", min = 0, max = 1;
            15037: U16, R_, "ALARM_EAF_RPM_ALARM", min = 0, max = 3;
            15038: U16, RW, "ALARM_EAF_RPM_CLEAR_ALARM", min = 0, max = 1;
            15042: SEC, R_, "ALARM_EAF_RPM_ERROR_DURATION_COUNTER";
            15057: U16, R_, "ALARM_FPT_ERROR", min = 0, max = 1;
            15058: U16, R_, "ALARM_FPT_ALARM", min = 0, max = 3;
            15059: U16, RW, "ALARM_FPT_CLEAR_ALARM", min = 0, max = 1;
            15063: SEC, R_, "ALARM_FPT_ERROR_DURATION_COUNTER";
            15064: U16, R_, "ALARM_OAT_ERROR", min = 0, max = 1;
            15065: U16, R_, "ALARM_OAT_ALARM", min = 0, max = 3;
            15066: U16, RW, "ALARM_OAT_CLEAR_ALARM", min = 0, max = 1;
            15070: SEC, R_, "ALARM_OAT_ERROR_DURATION_COUNTER";
            15071: U16, R_, "ALARM_SAT_ERROR", min = 0, max = 1;
            15072: U16, R_, "ALARM_SAT_ALARM", min = 0, max = 3;
            15073: U16, RW, "ALARM_SAT_CLEAR_ALARM", min = 0, max = 1;
            15077: SEC, R_, "ALARM_SAT_ERROR_DURATION_COUNTER";
            15078: U16, R_, "ALARM_RAT_ERROR", min = 0, max = 1;
            15079: U16, R_, "ALARM_RAT_ALARM", min = 0, max = 3;
            15080: U16, RW, "ALARM_RAT_CLEAR_ALARM", min = 0, max = 1;
            15084: SEC, R_, "ALARM_RAT_ERROR_DURATION_COUNTER";
            15085: U16, R_, "ALARM_EAT_ERROR", min = 0, max = 1;
            15086: U16, R_, "ALARM_EAT_ALARM", min = 0, max = 3;
            15087: U16, RW, "ALARM_EAT_CLEAR_ALARM", min = 0, max = 1;
            15091: SEC, R_, "ALARM_EAT_ERROR_DURATION_COUNTER";
            15092: U16, R_, "ALARM_ECT_ERROR", min = 0, max = 1;
            15093: U16, R_, "ALARM_ECT_ALARM", min = 0, max = 3;
            15094: U16, RW, "ALARM_ECT_CLEAR_ALARM", min = 0, max = 1;
            15098: SEC, R_, "ALARM_ECT_ERROR_DURATION_COUNTER";
            15099: U16, R_, "ALARM_EFT_ERROR", min = 0, max = 1;
            15100: U16, R_, "ALARM_EFT_ALARM", min = 0, max = 3;
            15101: U16, RW, "ALARM_EFT_CLEAR_ALARM", min = 0, max = 1;
            15105: SEC, R_, "ALARM_EFT_ERROR_DURATION_COUNTER";
            15106: U16, R_, "ALARM_OHT_ERROR", min = 0, max = 1;
            15107: U16, R_, "ALARM_OHT_ALARM", min = 0, max = 3;
            15108: U16, RW, "ALARM_OHT_CLEAR_ALARM", min = 0, max = 1;
            15112: SEC, R_, "ALARM_OHT_ERROR_DURATION_COUNTER";
            15113: U16, R_, "ALARM_EMT_ERROR", min = 0, max = 1;
            15114: U16, R_, "ALARM_EMT_ALARM", min = 0, max = 3;
            15115: U16, RW, "ALARM_EMT_CLEAR_ALARM", min = 0, max = 1;
            15119: SEC, R_, "ALARM_EMT_ERROR_DURATION_COUNTER";
            15120: U16, R_, "ALARM_RGS_ERROR", min = 0, max = 1;
            15121: U16, R_, "ALARM_RGS_ALARM", min = 0, max = 3;
            15122: U16, RW, "ALARM_RGS_CLEAR_ALARM", min = 0, max = 1;
            15126: SEC, R_, "ALARM_RGS_ERROR_DURATION_COUNTER";
            15127: U16, R_, "ALARM_BYS_ERROR", min = 0, max = 1;
            15128: U16, R_, "ALARM_BYS_ALARM", min = 0, max = 3;
            15129: U16, RW, "ALARM_BYS_CLEAR_ALARM", min = 0, max = 1;
            15133: SEC, R_, "ALARM_BYS_ERROR_DURATION_COUNTER";
            15134: U16, R_, "ALARM_SECONDARY_AIR_ERROR", min = 0, max = 1;
            15135: U16, R_, "ALARM_SECONDARY_AIR_ALARM", min = 0, max = 3;
            15136: U16, RW, "ALARM_SECONDARY_AIR_CLEAR_ALARM", min = 0, max = 1;
            15140: SEC, R_, "ALARM_SECONDARY_AIR_ERROR_DURATION_COUNTER";
            15141: U16, R_, "ALARM_FILTER_ERROR", min = 0, max = 1;
            15142: U16, R_, "ALARM_FILTER_ALARM", min = 0, max = 3;
            15143: U16, RW, "ALARM_FILTER_CLEAR_ALARM", min = 0, max = 1;
            15147: SEC, R_, "ALARM_FILTER_ERROR_DURATION_COUNTER";
            15148: U16, R_, "ALARM_EXTRA_CONTROLLER_ERROR", min = 0, max = 1;
            15149: U16, R_, "ALARM_EXTRA_CONTROLLER_ALARM", min = 0, max = 3;
            15150: U16, RW, "ALARM_EXTRA_CONTROLLER_CLEAR_ALARM", min = 0, max = 1;
            15154: SEC, R_, "ALARM_EXTRA_CONTROLLER_ERROR_DURATION_COUNTER";
            15155: U16, R_, "ALARM_EXTERNAL_STOP_ERROR", min = 0, max = 1;
            15156: U16, R_, "ALARM_EXTERNAL_STOP_ALARM", min = 0, max = 3;
            15157: U16, RW, "ALARM_EXTERNAL_STOP_CLEAR_ALARM", min = 0, max = 1;
            15161: SEC, R_, "ALARM_EXTERNAL_STOP_ERROR_DURATION_COUNTER";
            15162: U16, R_, "ALARM_RH_ERROR", min = 0, max = 1;
            15163: U16, R_, "ALARM_RH_ALARM", min = 0, max = 3;
            15164: U16, RW, "ALARM_RH_CLEAR_ALARM", min = 0, max = 1;
            15168: SEC, R_, "ALARM_RH_ERROR_DURATION_COUNTER";
            15169: U16, R_, "ALARM_CO2_ERROR", min = 0, max = 1;
            15170: U16, R_, "ALARM_CO2_ALARM", min = 0, max = 3;
            15171: U16, RW, "ALARM_CO2_CLEAR_ALARM", min = 0, max = 1;
            15175: SEC, R_, "ALARM_CO2_ERROR_DURATION_COUNTER";
            15176: U16, R_, "ALARM_LOW_SAT_ERROR", min = 0, max = 1;
            15177: U16, R_, "ALARM_LOW_SAT_ALARM", min = 0, max = 3;
            15178: U16, RW, "ALARM_LOW_SAT_CLEAR_ALARM", min = 0, max = 1;
            15182: SEC, R_, "ALARM_LOW_SAT_ERROR_DURATION_COUNTER";
            15183: U16, R_, "ALARM_BYF_ERROR", min = 0, max = 1;
            15184: U16, R_, "ALARM_BYF_ALARM";
            15185: U16, RW, "ALARM_BYF_CLEAR_ALARM", max = 0;
            15189: SEC, R_, "ALARM_BYF_ERROR_DURATION_COUNTER";
            15190: U16, R_, "ALARM_HEATER_MALFUNCTION_ERROR";
            15191: U16, R_, "ALARM_HEATER_MALFUNCTION_ALARM";
            15192: U16, R_, "ALARM_HEATER_MALFUNCTION_CLEAR_ALARM";
            15301: U16, R_, "ALARM_SAF_CTRL_TIMESTAMP_L";
            15302: U16, R_, "ALARM_SAF_CTRL_TIMESTAMP_H";
            15303: U16, R_, "ALARM_EAF_CTRL_TIMESTAMP_L";
            15304: U16, R_, "ALARM_EAF_CTRL_TIMESTAMP_H";
            15305: U16, R_, "ALARM_FROST_PROT_TIMESTAMP_L";
            15306: U16, R_, "ALARM_FROST_PROT_TIMESTAMP_H";
            15307: U16, R_, "ALARM_DEFROSTING_TIMESTAMP_L";
            15308: U16, R_, "ALARM_DEFROSTING_TIMESTAMP_H";
            15309: U16, R_, "ALARM_SAF_RPM_TIMESTAMP_L";
            15310: U16, R_, "ALARM_SAF_RPM_TIMESTAMP_H";
            15311: U16, R_, "ALARM_EAF_RPM_TIMESTAMP_L";
            15312: U16, R_, "ALARM_EAF_RPM_TIMESTAMP_H";
            15317: U16, R_, "ALARM_FPT_TIMESTAMP_L";
            15318: U16, R_, "ALARM_FPT_TIMESTAMP_H";
            15319: U16, R_, "ALARM_OAT_TIMESTAMP_L";
            15320: U16, R_, "ALARM_OAT_TIMESTAMP_H";
            15321: U16, R_, "ALARM_SAT_TIMESTAMP_L";
            15322: U16, R_, "ALARM_SAT_TIMESTAMP_H";
            15323: U16, R_, "ALARM_RAT_TIMESTAMP_L";
            15324: U16, R_, "ALARM_RAT_TIMESTAMP_H";
            15325: U16, R_, "ALARM_EAT_TIMESTAMP_L";
            15326: U16, R_, "ALARM_EAT_TIMESTAMP_H";
            15327: U16, R_, "ALARM_ECT_TIMESTAMP_L";
            15328: U16, R_, "ALARM_ECT_TIMESTAMP_H";
            15329: U16, R_, "ALARM_EFT_TIMESTAMP_L";
            15330: U16, R_, "ALARM_EFT_TIMESTAMP_H";
            15331: U16, R_, "ALARM_OHT_TIMESTAMP_L";
            15332: U16, R_, "ALARM_OHT_TIMESTAMP_H";
            15333: U16, R_, "ALARM_EMT_TIMESTAMP_L";
            15334: U16, R_, "ALARM_EMT_TIMESTAMP_H";
            15335: U16, R_, "ALARM_RGS_TIMESTAMP_L";
            15336: U16, R_, "ALARM_RGS_TIMESTAMP_H";
            15337: U16, R_, "ALARM_BYS_TIMESTAMP_L";
            15338: U16, R_, "ALARM_BYS_TIMESTAMP_H";
            15339: U16, R_, "ALARM_SECONDARY_AIR_TIMESTAMP_L";
            15340: U16, R_, "ALARM_SECONDARY_AIR_TIMESTAMP_H";
            15341: U16, R_, "ALARM_FILTER_TIMESTAMP_L";
            15342: U16, R_, "ALARM_FILTER_TIMESTAMP_H";
            15343: U16, R_, "ALARM_EXTRA_CONTROLLER_TIMESTAMP_L";
            15344: U16, R_, "ALARM_EXTRA_CONTROLLER_TIMESTAMP_H";
            15345: U16, R_, "ALARM_EXTERNAL_STOP_TIMESTAMP_L";
            15346: U16, R_, "ALARM_EXTERNAL_STOP_TIMESTAMP_H";
            15347: U16, R_, "ALARM_RH_TIMESTAMP_L";
            15348: U16, R_, "ALARM_RH_TIMESTAMP_H";
            15349: U16, R_, "ALARM_CO2_TIMESTAMP_L";
            15350: U16, R_, "ALARM_CO2_TIMESTAMP_H";
            15351: U16, R_, "ALARM_LOW_SAT_TIMESTAMP_L";
            15352: U16, R_, "ALARM_LOW_SAT_TIMESTAMP_H";
            15353: U16, R_, "ALARM_FIRE_ALARM_TIMESTAMP_L";
            15354: U16, R_, "ALARM_FIRE_ALARM_TIMESTAMP_H";
            15355: U16, R_, "ALARM_BYF_TIMESTAMP_L";
            15356: U16, R_, "ALARM_BYF_TIMESTAMP_H";
            15357: U16, R_, "ALARM_HEATER_MALFUNCTION_TIMESTAMP_L";
            15358: U16, R_, "ALARM_HEATER_MALFUNCTION_TIMESTAMP_H";
            15501: U16, R_, "ALARM_MANUAL_OVERRIDE_OUTPUTS_ERROR";
            15502: U16, R_, "ALARM_MANUAL_OVERRIDE_OUTPUTS_ALARM", min = 0, max = 3;
            15503: U16, RW, "ALARM_MANUAL_OVERRIDE_OUTPUTS_CLEAR_ALARM", min = 0, max = 1;
            15508: U16, R_, "ALARM_PDM_RHS_ERROR", min = 0, max = 1;
            15509: U16, R_, "ALARM_PDM_RHS_ALARM", min = 0, max = 3;
            15510: U16, RW, "ALARM_PDM_RHS_CLEAR_ALARM", min = 0, max = 1;
            15514: SEC, R_, "ALARM_PDM_RHS_ERROR_DURATION_COUNTER";
            15515: U16, R_, "ALARM_PDM_EAT_ERROR", min = 0, max = 1;
            15516: U16, R_, "ALARM_PDM_EAT_ALARM", min = 0, max = 3;
            15517: U16, RW, "ALARM_PDM_EAT_CLEAR_ALARM", min = 0, max = 1;
            15521: SEC, R_, "ALARM_PDM_EAT_ERROR_DURATION_COUNTER";
            15522: U16, R_, "ALARM_MANUAL_FAN_STOP_ERROR";
            15523: U16, R_, "ALARM_MANUAL_FAN_STOP_ALARM", min = 0, max = 3;
            15524: U16, RW, "ALARM_MANUAL_FAN_STOP_CLEAR_ALARM", min = 0, max = 1;
            15529: U16, R_, "ALARM_OVERHEAT_TEMPERATURE_ERROR", min = 0, max = 1;
            15530: U16, R_, "ALARM_OVERHEAT_TEMPERATURE_ALARM", min = 0, max = 3;
            15531: U16, RW, "ALARM_OVERHEAT_TEMPERATURE_CLEAR_ALARM", min = 0, max = 1;
            15535: SEC, R_, "ALARM_OVERHEAT_TEMPERATURE_ERROR_DURATION_COUNTER";
            15536: U16, R_, "ALARM_FIRE_ALARM_ERROR", min = 0, max = 1;
            15537: U16, R_, "ALARM_FIRE_ALARM_ALARM", min = 0, max = 3;
            15538: U16, RW, "ALARM_FIRE_ALARM_CLEAR_ALARM", min = 0, max = 1;
            15542: SEC, R_, "ALARM_FIRE_ALARM_ERROR_DURATION_COUNTER";
            15543: U16, R_, "ALARM_FILTER_WARNING_ERROR", min = 0, max = 1;
            15544: U16, R_, "ALARM_FILTER_WARNING_ALARM", min = 0, max = 3;
            15545: U16, RW, "ALARM_FILTER_WARNING_CLEAR_ALARM", min = 0, max = 1;
            15549: U16, R_, "ALARM_FILTER_WARNING_ALARM_ERROR_DURATION_COUNTER";
            15601: U16, R_, "ALARM_MANUAL_OVERRIDE_OUTPUTS_TIMESTAMP_L";
            15602: U16, R_, "ALARM_MANUAL_OVERRIDE_OUTPUTS_TIMESTAMP_H";
            15603: U16, R_, "ALARM_PDM_RHS_TIMESTAMP_L";
            15604: U16, R_, "ALARM_PDM_RHS_TIMESTAMP_H";
            15605: U16, R_, "ALARM_PDM_EAT_TIMESTAMP_L";
            15606: U16, R_, "ALARM_PDM_EAT_TIMESTAMP_H";
            15607: U16, R_, "ALARM_MANUAL_FAN_STOP_TIMESTAMP_L";
            15608: U16, R_, "ALARM_MANUAL_FAN_STOP_TIMESTAMP_H";
            15609: U16, R_, "ALARM_OVERHEAT_TEMPERATURE_TIMESTAMP_L";
            15610: U16, R_, "ALARM_OVERHEAT_TEMPERATURE_TIMESTAMP_H";
            15611: U16, R_, "ALARM_FILTER_WARNING_TIMESTAMP_L";
            15612: U16, R_, "ALARM_FILTER_WARNING_TIMESTAMP_H";
            15701: U16, R_, "ALARM_LOG_1_ID";
            15702: U16, R_, "ALARM_LOG_1_STATE_NOW";
            15703: U16, R_, "ALARM_LOG_1_STATE_PREVIOUS";
            15704: U16, R_, "ALARM_LOG_1_YEAR";
            15705: U16, R_, "ALARM_LOG_1_MONTH";
            15706: U16, R_, "ALARM_LOG_1_DAY";
            15707: U16, R_, "ALARM_LOG_1_HOUR";
            15708: U16, R_, "ALARM_LOG_1_MINUTE";
            15709: U16, R_, "ALARM_LOG_1_SECOND";
            15710: U16, R_, "ALARM_LOG_1_CODE";
            15711: U16, R_, "ALARM_LOG_2_ID";
            15712: U16, R_, "ALARM_LOG_2_STATE_NOW";
            15713: U16, R_, "ALARM_LOG_2_STATE_PREVIOUS";
            15714: U16, R_, "ALARM_LOG_2_YEAR";
            15715: U16, R_, "ALARM_LOG_2_MONTH";
            15716: U16, R_, "ALARM_LOG_2_DAY";
            15717: U16, R_, "ALARM_LOG_2_HOUR";
            15718: U16, R_, "ALARM_LOG_2_MINUTE";
            15719: U16, R_, "ALARM_LOG_2_SECOND";
            15720: U16, R_, "ALARM_LOG_2_CODE";
            15721: U16, R_, "ALARM_LOG_3_ID";
            15722: U16, R_, "ALARM_LOG_3_STATE_NOW";
            15723: U16, R_, "ALARM_LOG_3_STATE_PREVIOUS";
            15724: U16, R_, "ALARM_LOG_3_YEAR";
            15725: U16, R_, "ALARM_LOG_3_MONTH";
            15726: U16, R_, "ALARM_LOG_3_DAY";
            15727: U16, R_, "ALARM_LOG_3_HOUR";
            15728: U16, R_, "ALARM_LOG_3_MINUTE";
            15729: U16, R_, "ALARM_LOG_3_SECOND";
            15730: U16, R_, "ALARM_LOG_3_CODE";
            15731: U16, R_, "ALARM_LOG_4_ID";
            15732: U16, R_, "ALARM_LOG_4_STATE_NOW";
            15733: U16, R_, "ALARM_LOG_4_STATE_PREVIOUS";
            15734: U16, R_, "ALARM_LOG_4_YEAR";
            15735: U16, R_, "ALARM_LOG_4_MONTH";
            15736: U16, R_, "ALARM_LOG_4_DAY";
            15737: U16, R_, "ALARM_LOG_4_HOUR";
            15738: U16, R_, "ALARM_LOG_4_MINUTE";
            15739: U16, R_, "ALARM_LOG_4_SECOND";
            15740: U16, R_, "ALARM_LOG_4_CODE";
            15741: U16, R_, "ALARM_LOG_5_ID";
            15742: U16, R_, "ALARM_LOG_5_STATE_NOW";
            15743: U16, R_, "ALARM_LOG_5_STATE_PREVIOUS";
            15744: U16, R_, "ALARM_LOG_5_YEAR";
            15745: U16, R_, "ALARM_LOG_5_MONTH";
            15746: U16, R_, "ALARM_LOG_5_DAY";
            15747: U16, R_, "ALARM_LOG_5_HOUR";
            15748: U16, R_, "ALARM_LOG_5_MINUTE";
            15749: U16, R_, "ALARM_LOG_5_SECOND";
            15750: U16, R_, "ALARM_LOG_5_CODE";
            15751: U16, R_, "ALARM_LOG_6_ID";
            15752: U16, R_, "ALARM_LOG_6_STATE_NOW";
            15753: U16, R_, "ALARM_LOG_6_STATE_PREVIOUS";
            15754: U16, R_, "ALARM_LOG_6_YEAR";
            15755: U16, R_, "ALARM_LOG_6_MONTH";
            15756: U16, R_, "ALARM_LOG_6_DAY";
            15757: U16, R_, "ALARM_LOG_6_HOUR";
            15758: U16, R_, "ALARM_LOG_6_MINUTE";
            15759: U16, R_, "ALARM_LOG_6_SECOND";
            15760: U16, R_, "ALARM_LOG_6_CODE";
            15761: U16, R_, "ALARM_LOG_7_ID";
            15762: U16, R_, "ALARM_LOG_7_STATE_NOW";
            15763: U16, R_, "ALARM_LOG_7_STATE_PREVIOUS";
            15764: U16, R_, "ALARM_LOG_7_YEAR";
            15765: U16, R_, "ALARM_LOG_7_MONTH";
            15766: U16, R_, "ALARM_LOG_7_DAY";
            15767: U16, R_, "ALARM_LOG_7_HOUR";
            15768: U16, R_, "ALARM_LOG_7_MINUTE";
            15769: U16, R_, "ALARM_LOG_7_SECOND";
            15770: U16, R_, "ALARM_LOG_7_CODE";
            15771: U16, R_, "ALARM_LOG_8_ID";
            15772: U16, R_, "ALARM_LOG_8_STATE_NOW";
            15773: U16, R_, "ALARM_LOG_8_STATE_PREVIOUS";
            15774: U16, R_, "ALARM_LOG_8_YEAR";
            15775: U16, R_, "ALARM_LOG_8_MONTH";
            15776: U16, R_, "ALARM_LOG_8_DAY";
            15777: U16, R_, "ALARM_LOG_8_HOUR";
            15778: U16, R_, "ALARM_LOG_8_MINUTE";
            15779: U16, R_, "ALARM_LOG_8_SECOND";
            15780: U16, R_, "ALARM_LOG_8_CODE";
            15781: U16, R_, "ALARM_LOG_9_ID";
            15782: U16, R_, "ALARM_LOG_9_STATE_NOW";
            15783: U16, R_, "ALARM_LOG_9_STATE_PREVIOUS";
            15784: U16, R_, "ALARM_LOG_9_YEAR";
            15785: U16, R_, "ALARM_LOG_9_MONTH";
            15786: U16, R_, "ALARM_LOG_9_DAY";
            15787: U16, R_, "ALARM_LOG_9_HOUR";
            15788: U16, R_, "ALARM_LOG_9_MINUTE";
            15789: U16, R_, "ALARM_LOG_9_SECOND";
            15790: U16, R_, "ALARM_LOG_9_CODE";
            15791: U16, R_, "ALARM_LOG_10_ID";
            15792: U16, R_, "ALARM_LOG_10_STATE_NOW";
            15793: U16, R_, "ALARM_LOG_10_STATE_PREVIOUS";
            15794: U16, R_, "ALARM_LOG_10_YEAR";
            15795: U16, R_, "ALARM_LOG_10_MONTH";
            15796: U16, R_, "ALARM_LOG_10_DAY";
            15797: U16, R_, "ALARM_LOG_10_HOUR";
            15798: U16, R_, "ALARM_LOG_10_MINUTE";
            15799: U16, R_, "ALARM_LOG_10_SECOND";
            15800: U16, R_, "ALARM_LOG_10_CODE";
            15801: U16, R_, "ALARM_LOG_11_ID";
            15802: U16, R_, "ALARM_LOG_11_STATE_NOW";
            15803: U16, R_, "ALARM_LOG_11_STATE_PREVIOUS";
            15804: U16, R_, "ALARM_LOG_11_YEAR";
            15805: U16, R_, "ALARM_LOG_11_MONTH";
            15806: U16, R_, "ALARM_LOG_11_DAY";
            15807: U16, R_, "ALARM_LOG_11_HOUR";
            15808: U16, R_, "ALARM_LOG_11_MINUTE";
            15809: U16, R_, "ALARM_LOG_11_SECOND";
            15810: U16, R_, "ALARM_LOG_11_CODE";
            15811: U16, R_, "ALARM_LOG_12_ID";
            15812: U16, R_, "ALARM_LOG_12_STATE_NOW";
            15813: U16, R_, "ALARM_LOG_12_STATE_PREVIOUS";
            15814: U16, R_, "ALARM_LOG_12_YEAR";
            15815: U16, R_, "ALARM_LOG_12_MONTH";
            15816: U16, R_, "ALARM_LOG_12_DAY";
            15817: U16, R_, "ALARM_LOG_12_HOUR";
            15818: U16, R_, "ALARM_LOG_12_MINUTE";
            15819: U16, R_, "ALARM_LOG_12_SECOND";
            15820: U16, R_, "ALARM_LOG_12_CODE";
            15821: U16, R_, "ALARM_LOG_13_ID";
            15822: U16, R_, "ALARM_LOG_13_STATE_NOW";
            15823: U16, R_, "ALARM_LOG_13_STATE_PREVIOUS";
            15824: U16, R_, "ALARM_LOG_13_YEAR";
            15825: U16, R_, "ALARM_LOG_13_MONTH";
            15826: U16, R_, "ALARM_LOG_13_DAY";
            15827: U16, R_, "ALARM_LOG_13_HOUR";
            15828: U16, R_, "ALARM_LOG_13_MINUTE";
            15829: U16, R_, "ALARM_LOG_13_SECOND";
            15830: U16, R_, "ALARM_LOG_13_CODE";
            15831: U16, R_, "ALARM_LOG_14_ID";
            15832: U16, R_, "ALARM_LOG_14_STATE_NOW";
            15833: U16, R_, "ALARM_LOG_14_STATE_PREVIOUS";
            15834: U16, R_, "ALARM_LOG_14_YEAR";
            15835: U16, R_, "ALARM_LOG_14_MONTH";
            15836: U16, R_, "ALARM_LOG_14_DAY";
            15837: U16, R_, "ALARM_LOG_14_HOUR";
            15838: U16, R_, "ALARM_LOG_14_MINUTE";
            15839: U16, R_, "ALARM_LOG_14_SECOND";
            15840: U16, R_, "ALARM_LOG_14_CODE";
            15841: U16, R_, "ALARM_LOG_15_ID";
            15842: U16, R_, "ALARM_LOG_15_STATE_NOW";
            15843: U16, R_, "ALARM_LOG_15_STATE_PREVIOUS";
            15844: U16, R_, "ALARM_LOG_15_YEAR";
            15845: U16, R_, "ALARM_LOG_15_MONTH";
            15846: U16, R_, "ALARM_LOG_15_DAY";
            15847: U16, R_, "ALARM_LOG_15_HOUR";
            15848: U16, R_, "ALARM_LOG_15_MINUTE";
            15849: U16, R_, "ALARM_LOG_15_SECOND";
            15850: U16, R_, "ALARM_LOG_15_CODE";
            15851: U16, R_, "ALARM_LOG_16_ID";
            15852: U16, R_, "ALARM_LOG_16_STATE_NOW";
            15853: U16, R_, "ALARM_LOG_16_STATE_PREVIOUS";
            15854: U16, R_, "ALARM_LOG_16_YEAR";
            15855: U16, R_, "ALARM_LOG_16_MONTH";
            15856: U16, R_, "ALARM_LOG_16_DAY";
            15857: U16, R_, "ALARM_LOG_16_HOUR";
            15858: U16, R_, "ALARM_LOG_16_MINUTE";
            15859: U16, R_, "ALARM_LOG_16_SECOND";
            15860: U16, R_, "ALARM_LOG_16_CODE";
            15861: U16, R_, "ALARM_LOG_17_ID";
            15862: U16, R_, "ALARM_LOG_17_STATE_NOW";
            15863: U16, R_, "ALARM_LOG_17_STATE_PREVIOUS";
            15864: U16, R_, "ALARM_LOG_17_YEAR";
            15865: U16, R_, "ALARM_LOG_17_MONTH";
            15866: U16, R_, "ALARM_LOG_17_DAY";
            15867: U16, R_, "ALARM_LOG_17_HOUR";
            15868: U16, R_, "ALARM_LOG_17_MINUTE";
            15869: U16, R_, "ALARM_LOG_17_SECOND";
            15870: U16, R_, "ALARM_LOG_17_CODE";
            15871: U16, R_, "ALARM_LOG_18_ID";
            15872: U16, R_, "ALARM_LOG_18_STATE_NOW";
            15873: U16, R_, "ALARM_LOG_18_STATE_PREVIOUS";
            15874: U16, R_, "ALARM_LOG_18_YEAR";
            15875: U16, R_, "ALARM_LOG_18_MONTH";
            15876: U16, R_, "ALARM_LOG_18_DAY";
            15877: U16, R_, "ALARM_LOG_18_HOUR";
            15878: U16, R_, "ALARM_LOG_18_MINUTE";
            15879: U16, R_, "ALARM_LOG_18_SECOND";
            15880: U16, R_, "ALARM_LOG_18_CODE";
            15881: U16, R_, "ALARM_LOG_19_ID";
            15882: U16, R_, "ALARM_LOG_19_STATE_NOW";
            15883: U16, R_, "ALARM_LOG_19_STATE_PREVIOUS";
            15884: U16, R_, "ALARM_LOG_19_YEAR";
            15885: U16, R_, "ALARM_LOG_19_MONTH";
            15886: U16, R_, "ALARM_LOG_19_DAY";
            15887: U16, R_, "ALARM_LOG_19_HOUR";
            15888: U16, R_, "ALARM_LOG_19_MINUTE";
            15889: U16, R_, "ALARM_LOG_19_SECOND";
            15890: U16, R_, "ALARM_LOG_19_CODE";
            15891: U16, R_, "ALARM_LOG_20_ID";
            15892: U16, R_, "ALARM_LOG_20_STATE_NOW";
            15893: U16, R_, "ALARM_LOG_20_STATE_PREVIOUS";
            15894: U16, R_, "ALARM_LOG_20_YEAR";
            15895: U16, R_, "ALARM_LOG_20_MONTH";
            15896: U16, R_, "ALARM_LOG_20_DAY";
            15897: U16, R_, "ALARM_LOG_20_HOUR";
            15898: U16, R_, "ALARM_LOG_20_MINUTE";
            15899: U16, R_, "ALARM_LOG_20_SECOND";
            15900: U16, R_, "ALARM_LOG_20_CODE";
            15901: U16, R_, "ALARM_TYPE_A", min = 0, max = 1;
            15902: U16, R_, "ALARM_TYPE_B", min = 0, max = 1;
            15903: U16, R_, "ALARM_TYPE_C", min = 0, max = 1;
            15905: U16, RW, "ALARM_CLEAR_ALL", min = 0, max = 1;
            15906: U16, R_, "ALARM_AMOUNT_ACTIVE";
            15907: U16, R_, "ALARM_MODBUS_SUM", min = 0, max = 2;
            15910: U16, RW, "ALARM_LOW_SAT_ENABLED", min = 0, max = 1;
            16001: U16, RW, "PASSWD_ADMIN";
            16002: U16, RW, "LOCKED_USER", min = 0, max = 1;
            16003: U16, RW, "LOCKED_FILTER", min = 0, max = 1;
            16004: U16, RW, "LOCKED_WEEK_SCHEDULE", min = 0, max = 1;
            16051: U16, RW, "PASSWD_USER_LEVEL_REQUIRED", min = 0, max = 1;
            16052: U16, RW, "PASSWD_FILTER_REQUIRED", min = 0, max = 1;
            16053: U16, RW, "PASSWD_WEEK_SCHEDULE_REQUIRED", min = 0, max = 1;
            16061: U16, RW, "PASSWD_PC_SETTINGS";
            16062: U16, R_, "PASSWD_PC_UNLOCKED", min = 0, max = 1;
            16101: U16, RW, "SUW_REQUIRED", min = 0, max = 1;
            17001: U16, RW, "COMM_MODBUS_ADDRESS", min = 0, max = 255;
            17002: U16, RW, "COMM_MODBUS_BAUD_RATE", min = 0, max = 10;
            17003: U16, RW, "COMM_MODBUS_PARITY", min = 0, max = 2;
            17004: U16, RW, "COMM_MODBUS_STOP_BITS";
            17005: U16, R_, "COMM_MODBUS_SMARTLY_GATEWAY_STATE";
            17101: U16, RW, "COMM_DEBUG_SET_TO_STREAMING";
            17102: U16, RW, "COMM_M3B_SET_TO_MODBUS";
            20001: U16, R_, "PU_RUNNING_VERSION_MAJOR";
            20002: U16, R_, "PU_RUNNING_VERSION_MINOR";
            20003: U16, R_, "PU_RUNNING_VERSION_BUILD";
            20004: U16, R_, "PU_RUNNING_RESOURCES_MAJOR";
            20005: U16, R_, "PU_RUNNING_RESOURCES_MINOR";
            20006: U16, R_, "PU_RUNNING_RESOURCES_BUILD";
            20007: U16, R_, "PU_FLASH_VERSION_MB_MAJOR";
            20008: U16, R_, "PU_FLASH_VERSION_MB_MINOR";
            20009: U16, R_, "PU_FLASH_VERSION_MB_BUILD";
            20010: U16, R_, "PU_FLASH_VERSION_HMI_MAJOR";
            20011: U16, R_, "PU_FLASH_VERSION_HMI_MINOR";
            20012: U16, R_, "PU_FLASH_VERSION_HMI_BUILD";
            20013: U16, R_, "PU_FLASH_HMI_RESOURCES_MAJOR";
            20014: U16, R_, "PU_FLASH_HMI_RESOURCES_MINOR";
            20015: U16, R_, "PU_FLASH_HMI_RESOURCES_BUILD";
            20016: U16, R_, "PU_UPDATE_SOURCE";
            20544: U16, R_, "PU_UPDATE_MISC";
            21067: U16, R_, "PU_FLASH_VERSION_MB_L486_MAJOR";
            21068: U16, R_, "PU_FLASH_VERSION_MB_L486_MINOR";
            21069: U16, R_, "PU_FLASH_VERSION_MB_L486_BUILD";
            21071: U16, R_, "PARAMETERS_VERSION";
            21072: U16, R_, "PU_FLASH_VERSION_HMI_STM_MAJOR";
            21073: U16, R_, "PU_FLASH_VERSION_HMI_STM_MINOR";
            21074: U16, R_, "PU_FLASH_VERSION_HMI_STM_BUILD";
            21075: U16, R_, "PU_FLASH_VERSION_MB_G473_MAJOR";
            21076: U16, R_, "PU_FLASH_VERSION_MB_G473_MINOR";
            21077: U16, R_, "PU_FLASH_VERSION_MB_G473_BUILD";
            23001: I16, R_, "PIGGYBACK_1_PRESSURE_SAF";
            23002: I16, R_, "PIGGYBACK_1_PRESSURE_EAF";
            23036: U16, RW, "PIGGYBACK_1_MODE";
            23039: U16, RW, "PIGGYBACK_1_SAF_COMPENSATION";
            23040: U16, RW, "PIGGYBACK_1_EAF_COMPENSATION";
            23136: U16, RW, "PIGGYBACK_2_MODE";
            30004: MV, R_, "DIRTY_NTC_VOLTAGE1";
            30005: MV, R_, "DIRTY_NTC_VOLTAGE2";
            30006: MV, R_, "DIRTY_NTC_VOLTAGE3";
            30007: MV, R_, "DIRTY_NTC_VOLTAGE4";
            30008: MV, R_, "DIRTY_NTC_VOLTAGE5";
            30009: MV, R_, "DIRTY_NTC_VOLTAGE6";
            30010: MV, R_, "DIRTY_NTC_VOLTAGE7";
            30061: U16, R_, "STAT_WDOG_RESET_COUNT_LSB";
            30062: U16, R_, "STAT_WDOG_RESET_COUNT_MSB";
            30063: U16, R_, "STAT_STACK_HIGH_WATER_MARK_A";
            30064: U16, R_, "STAT_STACK_HIGH_WATER_MARK_B";
            30065: U16, R_, "STAT_UPTIME_LSB";
            30066: U16, R_, "STAT_UPTIME_MSB";
            30067: U16, R_, "STAT_REBOOT_BREADCRUMB";
            30068: U16, R_, "STAT_BREADCRUMB_X1";
            30069: U16, R_, "STAT_BREADCRUMB_X2";
            30070: U16, R_, "STAT_BREADCRUMB_X3";
            30071: U16, R_, "STAT_BREADCRUMB_UPTIME_L";
            30072: U16, R_, "STAT_BREADCRUMB_UPTIME_H";
            30073: U16, R_, "STAT_BREADCRUMB_LINE_L";
            30074: U16, R_, "STAT_BREADCRUMB_LINE_H";
            30101: U16, RW, "FACTORY_RESET", min = 3228, max = 3228;
            30103: U16, RW, "SET_USER_SAFE_CONFIG", min = 0, max = 1;
            30104: U16, RW, "ACTIVATE_USER_SAFE_CONFIG", min = 0, max = 1;
            30105: U16, R_, "USER_SAFE_CONFIG_VALID";
            30106: U16, R_, "SAFE_CONFIG_VALID";
            49901: U16, RW, "LOG_REQ_CHANNEL";
            49902: U16, RW, "LOG_REQ_POINTS";
            49903: U16, RW, "LOG_REQ_INTERVAL";
            49904: U16, RW, "LOG_REQ_INDEX";
            49905: U16, RW, "LOG_REQ_START";
            49951: U16, R_, "LOG_STATUS_DEPTH";
            49952: U16, R_, "LOG_STATUS_DATA_VALID";
            50001: U16, R_, "LOG_VALUE_0";
            51001: U16, R_, "LOG_TIMESTAMP_0";
        }
    };
}

macro_rules! optional {
    () => {
        None
    };
    ($($lit: tt)+) => {
        Some($($lit)*)
    };
}

macro_rules! make_lists {
    ($($regnum: literal: $dt: ident, $mode: ident, $name: literal $(, min = $min: literal)? $(, max = $max: literal)?;)+) => {
        pub static ADDRESSES: &[u16] = &[$($regnum),*];
        pub static NAMES: &[&str] = &[$($name),*];
        pub static MODES: &[Mode] = &[$(Mode::$mode),*];
        pub static DATA_TYPES: &[DataType] = &[$(DataType::$dt),*];
        pub static MINIMUM_VALUES: &[Option<Value>] = &[$(optional!($(Value::$dt($min))?)),*];
        pub static MAXIMUM_VALUES: &[Option<Value>] = &[$(optional!($(Value::$dt($max))?)),*];
    };
}

for_each_register!(make_lists);

/// Mapping from an address back to a register index.
pub static ADDRESS_INDICES: &[u16; 65536] = &const {
    let mut array = [0xFFFF; 65536];
    let mut index = 0;
    macro_rules! make_indices {
        ($($regnum: literal: $dt: ident, $mode: ident, $name: literal $(, min = $min: literal)? $(, max = $max: literal)?;)+) => {
            $(array[$regnum] = index; index = index + 1;)+
        }
    }
    for_each_register!(make_indices);
    // last `index = index + 1` unrolled by `for_each_register!` makes the `unused_assignments`
    // lint fire.
    let _ = index;
    array
};

#[expect(clippy::manual_range_patterns)]
#[rustfmt::skip]
pub static DESCRIPTIONS: &[&str] = &const {
    let mut result = [""; ADDRESSES.len()];
    let mut index = 0;
    let mut previous_address = 0;
    while index < result.len() {
        let address = ADDRESSES[index];
        if address <= previous_address {
            panic!("ADDRESSES is not sorted (or has duplicate values)!");
        }
        previous_address = address;
        result[index] = match address {
            1001 => "Highest value of all RH sensors",
            1002 => "Highest value of all CO2 sensors",
            1011 => "Set point for RH demand control",
            1012 => "Sensor value for RH demand control",
            1019 => "Output value for RH demand control. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1021 => "Set point for CO2 demand control",
            1022 => "Sensor value for CO2 demand control",
            1029 => "Output value for CO2 demand control. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1031 => "Pband setting for RH demand control",
            1033 => "Set point setting for RH demand control winter time",
            1034 => "Set point setting for RH demand control summer time",
            1035 => "Flag indicating if RH demand control is allowed",
            1039 => "Actual season for Demand Control. 0=Summer, 1=Winter",
            1041 => "Pband setting for CO2 demand control",
            1043 => "Set point setting for CO2 demand control",
            1044 => "Flag indicating if CO2 demand control is allowed",
            1101 => "Time delay setting for user mode Holiday",
            1102 => "Time delay setting for user mode Away",
            1103 => "Time delay setting for user mode Fire Place",
            1104 => "Time delay setting for user mode Refresh",
            1105 => "Time delay setting for user mode Crowded",
            1111 | 1112 => "Remaining time for the state Holiday/Away/Fire Place/Refresh/Crowded",
            1121 => "Minimum level for Demand Control. 2=Low, 3=Normal",
            1122 => "Maximum level for user Demand Control. 3=Normal, 4=High, 5=Maximum",
            1123 => "Actual IAQ level. 0=Economic, 1=Good, 2=Improving",
            1131 => "Fan speed level for mode Manual. Applies to both the SAF and the EAF fan. 0=Off, 2=Low, 3=Normal, 4=High. Value Off only allowed if contents of register FAN_MANUAL_STOP_ALLOWED is 1.",
            1135 | 1136 => "Fan speed level for mode Crowded. 3=Normal, 4=High, 5=Maximum",
            1137 | 1138 => "Fan speed level for mode Refresh. 3=Normal, 4=High, 5=Maximum",
            1139 | 1140 => "Fan speed level for mode Fire Place. 1=Minimum, 2=Low, 3=Normal",
            1141 | 1142 => "Fan speed level for mode Away. Value Off only allowed if contents of register FAN_MANUAL_STOP_ALLOWED is 1.",
            1143 | 1144 => "Fan speed level for mode Holiday. Value Off only allowed if contents of register FAN_MANUAL_STOP_ALLOWED is 1.",
            1145 | 1146 => "Fan speed level for mode Cooker Hood. 1=Minimum, 2=Low, 3=Normal",
            1147 | 1148 => "Fan speed level for mode Vacuum Cleaner. 1=Minimum, 2=Low, 3=Normal",
            1151 => "Temperature setpoint offset for user mode Crowded",
            1161 => "Active User mode. 0=Auto, 1=Manual, 2=Crowded, 3=Refresh, 4=Fireplace, 5=Away, 6=Holiday, 7=Cooker Hood, 8=Vacuum Cleaner, 9=CDI1, 10=CDI2, 11=CDI3, 12=PressureGuard",
            1162 => "New desired user mode as requested by HMI. 0=None, 1=AUTO, 2=Manual, 3=Crowded, 4=Refresh, 5=Fireplace, 6=Away, 7=Holiday",
            1171 | 1172 | 1173 | 1174 | 1175 | 1176 => "Fan speed level for configurable digital input. 0=Off, 1=Minimum, 2=Low, 3=Normal, 4=High, 5=Maximum",
            1177 | 1178 => "Fan speed level for configurable pressure guard function. 0=Off, 1=Minimum, 2=Low, 3=Normal, 4=High, 5=Maximum",
            1181 | 1182 | 1183 | 1184 | 1185 | 1188 | 1189 | 1190 => "Off delay for DI",
            1221 => "SAF speed value for user mode Holiday. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1222 => "EAF speed value for user mode Holiday. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1223 => "SAF speed value for mode Cooker Hood. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1224 => "EAF speed value for mode Cooker Hood. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1225 => "SAF speed value for mode Vacuum Cleaner. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1226 => "EAF speed value for mode Vacuum Cleaner. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1227 | 1303 => "SAF speed value for low fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1228 | 1304 => "EAF speed value for low fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1251 => "Compensate only SF or both SF and EF. 0=SAF, 1=SAF/EAF",
            1252 => "Compensation value at lowest temperature.",
            1253 => "Temperature at which compensation reaches maximum value during the winter period. Not used since MB FW 1.6.0",
            1254 => "Temperature at which highest compensation is applied.",
            1255 => "Current outdoor compensation value",
            1256 => "Temperature at which compensation starts during the winter period.",
            1257 => "Temperature at which compensation starts during the summer period.",
            1258 => "Temperature at which compensation reaches maximum value during the summer period.",
            1259 => "Compensation value during summer period",
            1274 => "Type of fan control mode. 0=Percentage, 1=RPM, 2=VAV (Constant Pressure), 3=CAV (Constant Flow), 4=DCV (External)",
            1301 => "SAF speed value for minimum fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1302 => "EAF speed value for minimum fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1305 => "SAF speed value for normal fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1306 => "EAF speed value for normal fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1307 => "SAF speed value for high fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1308 => "EAF speed value for high fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1309 => "SAF speed value for maximum fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1310 => "EAF speed value for maximum fan speed. (1): Depends on regulation type. Value can be %, RPM, Pressure or Flow",
            1351 => "Indicates that both fans are running",
            1352 => "Indicates that the SAF shall be turned off once the electrical reheater is cooled down.",
            1353 => "Allow manual fan stop (also as selection for user modes and Week schedule). 0=Manual stop not allowed, 1=Manual stop allowed",
            1357 => "Electrical Heater hot counter. Count down from 120 sec.",
            1358 => "Supply Air Fan Speed Level After Heater Cooling Down",
            1359 => "Extract Air Fan Speed Level After Heater Cooling Down",
            1401 | 1411 | 1421 | 1431 => "SAF speed value for minimum fan speed",
            1402 | 1412 | 1422 | 1432 => "EAF speed value for minimum fan speed",
            1403 | 1413 | 1423 | 1433 => "SAF speed value for low fan speed",
            1404 | 1414 | 1424 | 1434 => "EAF speed value for low fan speed",
            1405 | 1415 | 1425 | 1435 => "SAF speed value for normal fan speed",
            1406 | 1416 | 1426 | 1436 => "EAF speed value for normal fan speed",
            1407 | 1417 | 1427 | 1437 => "SAF speed value for high fan speed",
            1408 | 1418 | 1428 | 1438 => "EAF speed value for high fan speed",
            1409 | 1419 | 1429 | 1439 => "SAF speed value for maximum fan speed",
            1410 | 1420 | 1430 | 1440 => "EAF speed value for maximum fan speed",
            1621 | 1623 | 1625 => "Remaining time",
            2001 | 2054 => "Temperature setpoint for the supply air temperature",
            2013 | 2053 => "Temperature set point for SATC, as calculated by RATC/EATC during cascade control",
            2021 => "Minimum temperature set point for the SATC",
            2022 => "Maximum temperature set point for the SATC",
            2031 => "Unit temperature control mode. 0=Supply, 1=Room, 2=Extract",
            2051 => "EAT or RAT value, used for room/extract air controller.",
            2055 => "Output of the SATC (0-100%)",
            2061 => "SATC setpoint value",
            2069 => "SATC output signal",
            2071 => "RATC setpoint value",
            2079 => "RATC output signal",
            2101 => "Value from External Controller Input, SAF",
            2102 => "Value from External Controller Input, EAF",
            2113 => "Temperature at which the Heater circulation pump is started",
            2122 => "Off time delay for the heater circulation pump in minutes",
            2134 => "Enabling of cooling recovery",
            2147 => "Indicates if RH transfer control shall be applied",
            2148 => "As the heat exchanger unit is rotates faster, more moisture returns indoors. If there is a demand to eject moisture outdoors, the rotor speed is limited. This register determines the maximum speed of the rotor due to moisture transfer.",
            2149 => "Heater TRIAC after manual override",
            2201 => "Pband setting for RH transfer control",
            2202 => "Itime setting for RH transfer control",
            2203 => "Set point setting for RH transfer control",
            2204 => "Enabling of humidity transfer control",
            2211 => "Extract air specific humidity (g/kg)",
            2212 => "Outdoor air specific humidity at assumed 90% RH outdoors",
            2213 => "Moisture transfer control specific humidity setpoint",
            2311 => "Cooler signal",
            2314 => "Temperature at which the cooler circulation pump is started",
            2315 => "Temperature at which cooling recovery is allowed",
            2316 => "Temperature at which cooling is interlocked",
            2317 => "Off time delay for the cooler circulation pump in minutes",
            2403 => "Set point value for the extra controller PI regulator",
            2404 => "Start temperature for extra controller circulation pump",
            2405 => "Off time delay for the extra controller circulation pump in minutes",
            2418 => "Temperature setpoint for the preheater. 0=Auto, 1=Manual",
            2451 => "Start temperature for the change-over circulation pump",
            2452 => "Off time delay for the change-over circulation pump in minutes",
            2504 => "Temperature offset for heating during Eco mode",
            2505 => "Enabling of eco mode",
            2506 => "Indicates if conditions for ECO mode are",
            3101 | 3102 | 3103 | 3104 | 3105 | 3106 | 3107 | 3108 | 3109 | 3110 | 3111 | 3112 | 3113 | 3114 | 3115 | 3116 | 3117 => "Is the function currently active?",
            4101 => "Indicates if free cooling is enabled",
            4102 => "Minimum of highest daytime temperature for start of free cooling.",
            4103 => "Highest night temperature limit for termination free cooling",
            4104 => "Lowest night temperature limit for termination free cooling",
            4105 => "Lowest temperature room temperature for termination of free cooling",
            4106 => "Start time of free cooling night-period, hour. Valid range is from 0 to 8 and from 21 to 23.",
            4107 => "Start time of free cooling night-period, Minute",
            4108 => "End time of free cooling night-period, hour. Valid range is from 0 to 8 and from 21 to 23.",
            4109 => "End time of free cooling night-period, Minute",
            4111 => "Indicates if free cooling is being performed",
            4112 => "Minimum speed level during free cooling, SAF. 3=Normal, 4=High, 5=Maximum",
            4113 => "Minimum speed level during free cooling, EAF. 3=Normal, 4=High, 5=Maximum",
            5001 => "Temperature offset during active week schedule.",
            5002 => "Temperature offset during inactive week schedule.",
            5003 => "Monday, Period 1, start",
            5005 => "Monday, Period 1, end",
            5007 => "Monday, Period 2, start",
            5009 => "Monday, Period 2, end",
            5011 => "Tuesday, Period 1, start",
            5013 => "Tuesday, Period 1, end",
            5015 => "Tuesday, Period 2, start",
            5017 => "Tuesday, Period 2, end",
            5019 => "Wednesday, Period 1, start",
            5021 => "Wednesday, Period 1, end",
            5023 => "Wednesday, Period 2, start",
            5025 => "Wednesday, Period 2, end",
            5027 => "Thursday, Period 1, start",
            5029 => "Thursday, Period 1, end",
            5031 => "Thursday, Period 2, start",
            5033 => "Thursday, Period 2, end",
            5035 => "Friday, Period 1, start",
            5037 => "Friday, Period 1, end",
            5039 => "Friday, Period 2, start",
            5041 => "Friday, Period 2, end",
            5043 => "Saturday, Period 1, start",
            5045 => "Saturday, Period 1, end",
            5047 => "Saturday, Period 2, start",
            5049 => "Saturday, Period 2, end",
            5051 => "Sunday, Period 1, start",
            5053 => "Sunday, Period 1, end",
            5055 => "Sunday, Period 2, start",
            5057 => "Sunday, Period 2, end",
            5059 => "Indicates that the current time lays within the indicated intervals",
            5060 => "Fan speed levels for SAF and EAF during active week schedule. 1=Off, 2=Low, 3=Normal, 4=High, 5=Demand. Off available if Manual Fan Stop is enabled. Demand available if demand control active or external fan control enabled.",
            5061 => "Fan speed levels for SAF and EAF during inactive week schedule. 1=Off, 2=Low, 3=Normal, 4=High, 5=Demand. Off available if Manual Fan Stop is enabled. Demand available if demand control active or external fan control enabled.",
            5101 | 5102 | 5103 | 5104 | 5105 | 5106 | 5107 | 5108 | 5109 | 5110 | 5111 | 5112 | 5113 | 5114 => "Flag indicating if this period is enabled.",
            6001 | 6002 | 6003 | 6004 | 6005 | 6006 => "Current time",
            6007 => "Flag indicating if DST is enabled. 0=Daylight saving time not enabled, 1=Daylight saving time enabled",
            6008 => "Indicates the presentation of time in the HMI. 0=12H, 1=24H",
            6009 => "Monday (0)...Sunday (6)",
            6011 => "Now time in seconds. Lower 16 bits.",
            6012 => "Now time in seconds. Higher 16 bits.",
            6101 => "RTC value in seconds, highest 16 bits",
            7001 => "Filter replacement time in months",
            7002 => "Timestamp of latest filter replcement, lower 16 bits",
            7003 => "Timestamp of latest filter replcement, higher 16 bits",
            7004 => "Indicates that the LastFilterReplacementTime shall be set to Now.",
            7005 => "Remaining filter time in seconds, lower 16 bits.",
            7006 => "Remaining filter time in seconds, higher 16 bits.",
            7007 => "Indicates if the filter warning alarm was generated.",
            9001 => "Unit for CAV control mode. 0=l/s, 1=m³/h, 2=cfm",
            9002 => "Units for VAV control mode. 0=Pa, 1=InH2O",
            9003 => "Units for temperature. 0=Celsius, 1=Fahrenheit",
            // Not in the public variable lists; inferred from the device and the SAVE CONNECT web
            // application's register enums.
            11001 | 11002 | 11003 | 11004 | 11005 | 11006 | 11007 => "Analog input function. 0=None, 1=OAT, 2=SAT, 3=OHT, 4=FPT, 5=RAT, 6=EAT, 7=ECT, 8=EFT",
            11021 | 11022 | 11023 | 11024 | 11025 | 11026 | 11027 => "Correction added to the analog input's temperature reading",
            11101 | 11102 | 11103 | 11104 | 11105 | 11106 => "Universal input type. 0=Not set, 1=Analog, 2=Digital",
            11201 | 11202 | 11203 | 11204 | 11205 | 11206 => "Universal input analog function. 0=None, 1=RH sensor, 2=CO2 sensor, 3=External supply fan control, 4=External extract fan control, 5=Bypass damper feedback",
            11401 | 11402 => "Indicates what kind of functionality is connected to the digital input. 0=None, 1=Away, 2=BYP, 3=Vacuum Cleaner, 4=Cooker Hood, 5=Crowded, 6=EMT, 7=External Stop, 8=Extra Controller Alarm, 9=Fireplace, 10=Holiday, 11=Refresh, 12=RGS, 13=Change Over Feedback, 14=Fire Alarm, 15=Configurable DI1, 16=Configurable DI2, 17=Configurable DI3, 18=Pressure Guard",
            11421 | 11422 => "Polarity of the digial input. 0=NO, 1=NC",
            12011 | 12012 | 12013 | 12014 | 12015 | 12016 => "Raw universal input voltage",
            12021 => "State of UI1",
            12022 => "State of UI2",
            12023 => "State of UI3",
            12024 => "State of UI4",
            12025 => "State of UI5",
            12026 => "State of UI6",
            12031 | 12032 => "Boolean",
            12101 => "Frost Protection Temperature sensor value (Water Heater)",
            12102 => "Outdoor Air Temperature sensor (standard)",
            12103 => "Supply Air Temperature sensor (standard)",
            12104 => "Room Air Temperature sensor (accessory)",
            12105 => "Extract Air Temperature sensor (accessory)",
            12106 => "Extra Controller Temperature sensor (accessory)",
            12107 => "Efficiency temperature sensor (accessory)",
            12108 => "Over Heat Temperature sensor (Electrical Heater)",
            12109 => "Relative Humidity Sensor (Accessory)",
            12112 => "Rotating guard Sensor input",
            12113 => "Set a virtual CO2 sensor reading",
            12114 => "Set a virtual RH sensor reading",
            12115 => "CO2 value (accessory)",
            12136 => "PDM RHS sensor value (standard)",
            12151 => "CO2 sensor value - UI1 (accessory)",
            12152 => "CO2 sensor value - UI2 (accessory)",
            12153 => "CO2 sensor value - UI3 (accessory)",
            12154 => "CO2 sensor value - UI4 (accessory)",
            12155 => "CO2 sensor value - UI5 (accessory)",
            12156 => "CO2 sensor value - UI6 (accessory)",
            12161 => "RH sensor value - UI1 (accessory)",
            12162 => "RH sensor value - UI2 (accessory)",
            12163 => "RH sensor value - UI3 (accessory)",
            12164 => "RH sensor value - UI4 (accessory)",
            12165 => "RH sensor value - UI5 (accessory)",
            12166 => "RH sensor value - UI6 (accessory)",
            12301 => "Value of physical Digital Input of Away function",
            12302 => "Value of physical Digital Input of Holiday function",
            12303 => "Value of physical Digital Input of Fireplace function",
            12304 => "Value of physical Digital Input of Refresh function",
            12305 => "Value of physical Digital Input of Crowded function",
            12306 => "Value of physical Digital Input of Cookerhood function",
            12307 => "Value of physical Digital Input of Vacuum Cleaner function",
            12308 => "External Stop input value",
            12309 => "Load Detected input value",
            12310 => "Extra controller EMT input value",
            12311 => "Fire Alarm input value",
            12312 => "Change over feedback value",
            12316 | 12317 | 12318 | 12319 => "Indicates if physical DI is active",
            12401 => "Supply Air Fan RPM indication from TACHO",
            12402 => "Extract Air Fan RPM indication from TACHO",
            12403 | 12404 => "Flow value calculated from piggyback pressure sensor.",
            12405 => "Value from Bypass Damper Feedback input",
            12544 => "PDM EAT sensor value (standard)",
            12929 | 12930 | 12931 | 12932 | 12933 | 12934 | 12935 | 12936 | 12937 | 12938 | 12939 | 12940 | 12941 | 12942 | 12943 | 12944 | 12945 | 12946 | 12947 | 12948 | 12949 | 12950 => "Enable manual override of the device input. 0=AUTO, 1=OVERRIDE",
            12951 | 12952 | 12953 | 12954 | 12955 | 12956 | 12957 | 12958 | 12959 | 12960 | 12961 | 12962 | 12963 | 12964 | 12979 | 12980 | 12983 | 12984 | 12985 | 12986 | 12987 | 12988 | 12989 | 12990 => "Value to override the device input with.",
            13201 => "Indicates if the TRIAC shall be used",
            13301 | 13302 | 13303 | 13304 => "Digital output after multiplexer",
            13311 | 13312 | 13313 | 13314 | 13315 => "Analog output after multiplexer",
            13601 => "SAF Override. 0=Auto, 1=Manual",
            13602 => "EAF Override. 0=Auto, 1=Manual",
            13801 => "SAF Override value in % if manual (1) selected",
            13802 => "EAF override value in % if manual (1) selected",
            14001 => "SAF fan speed",
            14002 => "EAF fan speed",
            14003 => "Sum Alarm DO. 0=Output not active, 1=Output active",
            14004 => "Indicates if Outdoor/Exhaust air damper signal is On/Off",
            14101 => "Heater AO state.",
            14102 => "Heater DO state. 0=Output not active, 1=Output active",
            14103 => "Heat Exchanger AO state.",
            14104 => "Heat Exchanger DO state. 0=Output not active, 1=Output active",
            14201 => "Cooler AO state.",
            14202 => "Cooler DO state. 0=Output not active, 1=Output active",
            14203 => "Extra controller AO state.",
            14204 => "Extra controller DO state: 0=Output not active, 1=Output active",
            14301 => "Heating circulation pump output",
            14302 => "Cooler circulation pump output",
            14303 => "Change-over circulation pump output",
            14304 => "Extra controller circulation pump output",
            14351 => "Voltage signal from AO1",
            14352 => "Voltage signal from AO2",
            14353 => "Voltage signal from AO3",
            14354 => "Voltage signal from AO4",
            14355 => "Voltage signal from AO5",
            14361 => "State of DO1",
            14362 => "State of DO2",
            14363 => "State of DO3",
            14364 => "State of DO4",
            14371 => "Supply air fan control signal",
            14372 => "Extract air fan control signal",
            14381 => "TRIAC control signal",
            15002 | 15009 | 15016 | 15023 | 15030 | 15037 | 15058 | 15065 | 15072 | 15079 | 15086 | 15093 | 15100 | 15107 | 15114 | 15121 | 15128 | 15135 | 15142 | 15149 | 15156 | 15163 | 15170 | 15177 | 15184 | 15502 | 15509 | 15516 | 15523 | 15530 | 15537 | 15544 => "Alarm active/inactive. 0=Inactive, 1=Active, 2=Waiting, 3=Cleared Error Active",
            15003 | 15010 | 15017 | 15024 | 15031 | 15038 | 15059 | 15066 | 15073 | 15080 | 15087 | 15094 | 15101 | 15108 | 15115 | 15122 | 15129 | 15136 | 15143 | 15150 | 15157 | 15164 | 15171 | 15178 | 15185 | 15503 | 15510 | 15517 | 15524 | 15531 | 15538 | 15545 => "Signal to clear the alarm",
            15549 => "Counter for delay",
            15701 | 15702 | 15703 | 15704 | 15705 | 15706 | 15707 | 15708 | 15709 | 15710 | 15711 | 15712 | 15713 | 15714 | 15715 | 15716 | 15717 | 15718 | 15719 | 15720 | 15721 | 15722 | 15723 | 15724 | 15725 | 15726 | 15727 | 15728 | 15729 | 15730 | 15731 | 15732 | 15733 | 15734 | 15735 | 15736 | 15737 | 15738 | 15739 | 15740 | 15741 | 15742 | 15743 | 15744 | 15745 | 15746 | 15747 | 15748 | 15749 | 15750 | 15751 | 15752 | 15753 | 15754 | 15755 | 15756 | 15757 | 15758 | 15759 | 15760 | 15761 | 15762 | 15763 | 15764 | 15765 | 15766 | 15767 | 15768 | 15769 | 15770 | 15771 | 15772 | 15773 | 15774 | 15775 | 15776 | 15777 | 15778 | 15779 | 15780 | 15781 | 15782 | 15783 | 15784 | 15785 | 15786 | 15787 | 15788 | 15789 | 15790 | 15791 | 15792 | 15793 | 15794 | 15795 | 15796 | 15797 | 15798 | 15799 | 15800 | 15801 | 15802 | 15803 | 15804 | 15805 | 15806 | 15807 | 15808 | 15809 | 15810 | 15811 | 15812 | 15813 | 15814 | 15815 | 15816 | 15817 | 15818 | 15819 | 15820 | 15821 | 15822 | 15823 | 15824 | 15825 | 15826 | 15827 | 15828 | 15829 | 15830 | 15831 | 15832 | 15833 | 15834 | 15835 | 15836 | 15837 | 15838 | 15839 | 15840 | 15841 | 15842 | 15843 | 15844 | 15845 | 15846 | 15847 | 15848 | 15849 | 15850 | 15851 | 15852 | 15853 | 15854 | 15855 | 15856 | 15857 | 15858 | 15859 | 15860 | 15861 | 15862 | 15863 | 15864 | 15865 | 15866 | 15867 | 15868 | 15869 | 15870 | 15871 | 15872 | 15873 | 15874 | 15875 | 15876 | 15877 | 15878 | 15879 | 15880 | 15881 | 15882 | 15883 | 15884 | 15885 | 15886 | 15887 | 15888 | 15889 | 15890 | 15891 | 15892 | 15893 | 15894 | 15895 | 15896 | 15897 | 15898 | 15899 | 15900 => "Alarm log information",
            15901 => "Indicates if an alarm Type A is active",
            15902 => "Indicates if an alarm Type B is active",
            15903 => "Indicates if an alarm Type C is active",
            16001 => "Administrator password. Bit 12-15: digit 1, Bit 8-11: digit 2, Bit 4-7: digit 3, Bit 0-3: digit 4",
            16002 => "Indicates if the User level is locked. 0=User menu locked, 1=User menu not locked",
            16003 => "Indicates if the Filter menu is locked. 0=menu locked, 1=menu not locked",
            16004 => "Indicates if the Week schedule menu is locked. 0=menu locked, 1=menu not locked",
            16051 => "Home screen lock",
            16052 => "Filter Change menu lock",
            16053 => "Week schedule menu lock",
            16101 => "Indicates if the start-up wizard shall be activated.",
            17001 => "Modbus address of the MB. Only relevant if the MB is a modbus slave.",
            17002 => "Baudrate of the modbus connection. 0=1200, 1=2400, 2=4800, 3=9600, 4=14400, 5=19200, 6=28800, 7=38400, 8=57600, 9=76800, 10=115200",
            17003 => "Parity setting for the modbus connection. 0=None, 1=Even, 2=Odd",
            30101 => "Activates setting of the parameters to their default values. Only activated by writing 3228 to this register.",
            1003 => "Supply air fan speed by demand control function",
            1004 => "Sensor type by which demand control function is operating. 0=CO₂, 1=RH",
            1007 => "Extract air fan speed by demand control function",
            1032 => "Demand control by RH I-time setting value",
            1036 => "Time until summer mode becomes active for demand control, lower 16 bits, s",
            1037 => "Time until summer mode becomes active for demand control, higher 16 bits, s",
            1038 => "Demand control mode by season. 0=Summer, 1=Winter, 2=Identifying",
            1042 => "Demand control by CO2 I-time setting value",
            1061 => "Status of demand control. 0=Disabled, 1=Enabled",
            1062 => "Auto mode type. 0=External control, 1=Demand control, 2=Week schedule, 3=Configuration fault",
            1164 => "Used mode in which SAVE unit should operate when user mode delay is expired. 0=Auto, 1=Manual",
            1201 => "Supply air fan speed in manual mode, in units of the fan regulation type",
            1202 => "Extract air fan speed in manual mode, in units of the fan regulation type",
            1203 => "Supply air fan speed in auto mode, in units of the fan regulation type",
            1204 => "Extract air fan speed in auto mode, in units of the fan regulation type",
            1205 => "Supply air fan speed in crowded mode, in units of the fan regulation type",
            1206 => "Extract air fan speed in crowded mode, in units of the fan regulation type",
            1207 => "Supply air fan speed in refresh mode, in units of the fan regulation type",
            1208 => "Extract air fan speed in refresh mode, in units of the fan regulation type",
            1209 => "Supply air fan speed in fireplace mode, in units of the fan regulation type",
            1210 => "Extract air fan speed in fireplace mode, in units of the fan regulation type",
            1211 => "Supply air fan speed in away mode, in units of the fan regulation type",
            1212 => "Extract air fan speed in away mode, in units of the fan regulation type",
            1213 => "Supply air fan speed in holiday mode, in units of the fan regulation type",
            1214 => "Extract air fan speed in holiday mode, in units of the fan regulation type",
            1215 => "Supply air fan speed in cooker hood mode, in units of the fan regulation type",
            1216 => "Extract air fan speed in cooker hood mode, in units of the fan regulation type",
            1217 => "Supply air fan speed in vacuum cleaner mode, in units of the fan regulation type",
            1218 => "Extract air fan speed in vacuum cleaner mode, in units of the fan regulation type",
            1271 => "Fan control P-band value. Applies only if fan is controlled by pressure, flow or RPM",
            1272 => "Fan control I-time value. Applies only if fan is controlled by pressure, flow or RPM",
            1551 => "Supply air fan operation time with output at 0–20 %, lower 16 bits, h",
            1552 => "Supply air fan operation time with output at 0–20 %, higher 16 bits, h",
            1553 => "Supply air fan operation time with output at 21–40 %, lower 16 bits, h",
            1554 => "Supply air fan operation time with output at 21–40 %, higher 16 bits, h",
            1555 => "Supply air fan operation time with output at 41–60 %, lower 16 bits, h",
            1556 => "Supply air fan operation time with output at 41–60 %, higher 16 bits, h",
            1557 => "Supply air fan operation time with output at 61–80 %, lower 16 bits, h",
            1558 => "Supply air fan operation time with output at 61–80 %, higher 16 bits, h",
            1559 => "Supply air fan operation time with output at 81–100 %, lower 16 bits, h",
            1560 => "Supply air fan operation time with output at 81–100 %, higher 16 bits, h",
            1561 => "Extract air fan operation time with output at 0–20 %, lower 16 bits, h",
            1562 => "Extract air fan operation time with output at 0–20 %, higher 16 bits, h",
            1563 => "Extract air fan operation time with output at 21–40 %, lower 16 bits, h",
            1564 => "Extract air fan operation time with output at 21–40 %, higher 16 bits, h",
            1565 => "Extract air fan operation time with output at 41–60 %, lower 16 bits, h",
            1566 => "Extract air fan operation time with output at 41–60 %, higher 16 bits, h",
            1567 => "Extract air fan operation time with output at 61–80 %, lower 16 bits, h",
            1568 => "Extract air fan operation time with output at 61–80 %, higher 16 bits, h",
            1569 => "Extract air fan operation time with output at 81–100 %, lower 16 bits, h",
            1570 => "Extract air fan operation time with output at 81–100 %, higher 16 bits, h",
            1601 => "Away mode remaining time, when activated by digital input, lower 16 bits, s",
            1602 => "Away mode remaining time, when activated by digital input, higher 16 bits, s",
            1603 => "Away mode remaining time, when activated using user interface, lower 16 bits, s",
            1604 => "Away mode remaining time, when activated using user interface, higher 16 bits, s",
            1605 => "Crowded mode remaining time, when activated by digital input, lower 16 bits, s",
            1606 => "Crowded mode remaining time, when activated by digital input, higher 16 bits, s",
            1607 => "Crowded mode remaining time, when activated using user interface, lower 16 bits, s",
            1608 => "Crowded mode remaining time, when activated using user interface, higher 16 bits, s",
            1609 => "Holiday mode remaining time, when activated by digital input, lower 16 bits, s",
            1610 => "Holiday mode remaining time, when activated by digital input, higher 16 bits, s",
            1611 => "Holiday mode remaining time, when activated using user interface, lower 16 bits, s",
            1612 => "Holiday mode remaining time, when activated using user interface, higher 16 bits, s",
            1613 => "Fireplace mode remaining time, when activated by digital input, lower 16 bits, s",
            1614 => "Fireplace mode remaining time, when activated by digital input, higher 16 bits, s",
            1615 => "Fireplace mode remaining time, when activated using user interface, lower 16 bits, s",
            1616 => "Fireplace mode remaining time, when activated using user interface, higher 16 bits, s",
            1617 => "Refresh mode remaining time, when activated by digital input, lower 16 bits, s",
            1618 => "Refresh mode remaining time, when activated by digital input, higher 16 bits, s",
            1619 => "Refresh mode remaining time, when activated using user interface, lower 16 bits, s",
            1620 => "Refresh mode remaining time, when activated using user interface, higher 16 bits, s",
            2011 => "Extract/room air temperature control P-band. Only used in extract/room air temperature control mode",
            2012 => "Extract/room air temperature control I-time. Only used in extract/room air temperature control mode",
            2041 => "Supply air temperature control P-band value",
            2042 => "Supply air temperature control I-time value",
            2114 => "Heating demand by temperature control",
            2124 => "Time till heating circulation pump stops",
            2141 => "Heat recovery demand by temperature control",
            2221 => "Rotary heat exchanger cleaning function status. 0=Inactive, 1=Active",
            2263 => "Passive house function status. 0=Disabled, 1=Enabled",
            2318 => "Time till cooling circulation pump stops",
            2401 => "Extra controller P-band value",
            2402 => "Extra controller I-time value",
            2419 => "Time till extra controller circulation pump stops",
            2424 => "Time until summer mode activates for the GEO heat exchanger, lower 16 bits, s",
            2425 => "Time until summer mode activates for the GEO heat exchanger, higher 16 bits, s",
            2426 => "Mode for GEO heat exchanger. 0=None, 1=Summer, 2=Winter",
            2427 => "Preheater deactivation by outside air temperature status. 0=Disabled, 1=Enabled",
            2428 => "Temperature at which preheater operation is allowed",
            2453 => "Time till change over circulation pump stops",
            2521 => "Operation mode of ECO mode. 0=Inactive, 1=Active",
            4001 => "Defrosting mode. 0=Soft, 1=Normal, 2=Hard",
            4003 => "Defrosting level",
            4011 => "Defrosting state. 0=Normal, 1=Bypass, 2=Stop, 3=Secondary air, 4=Error",
            4012 => "Time in specific defrosting state",
            4114 => "Free cooling state. 0=Disabled, 1=Enabled, 2=Daytime, 3=Temperatures are not reliable",
            4119 => "Time for heating block after free cooling",
            4120 => "Reliable temperature status. 0=Not reliable, 1=Reliable",
            12110 => "Bypass damper input status. 0=Inactive, 1=Active",
            12111 => "Emergency thermostat input status. 0=Inactive, 1=Active",
            12201 => "Supply air fan pressure, Pa",
            12202 => "Extract air fan pressure, Pa",
            12203 => "Supply air fan flow indication, l/s, m3h, cfm (depends on the unit configuration)",
            12204 => "Extract air fan flow indication, l/s, m3h, cfm (depends on the unit configuration)",
            14305 => "Activate cooling digital output status. 0=Inactive, 1=Active",
            14306 => "Change over analog output value",
            14307 => "Air recirculation digital output status. 0=Inactive, 1=Active",
            14308 => "External fan control block digital output status. 0=Unblocked, 1=Blocked",
            14309 => "Digital output for unit status. 0=Active alarms detected, 1=Unit status OK",
            14310 => "Week schedule unscheduled period digital output status. 0=Inactive, 1=Active",
            14311 => "Week schedule scheduled period digital output status. 0=Inactive, 1=Active",
            14401 => "K factor value for supply air fan. Applies only if fan is controlled by flow",
            14402 => "K factor value for extract air fan. Applies only if fan is controlled by flow",
            15001 => "Supply air fan control alarm return status. 0=Returned, 1=Not returned",
            15007 => "Supply air fan control alarm counter: how long the alarm condition has lasted",
            15008 => "Extract air fan control alarm return status. 0=Returned, 1=Not returned",
            15014 => "Extract air fan control alarm counter: how long the alarm condition has lasted",
            15015 => "Frost protection alarm return status. 0=Returned, 1=Not returned",
            15021 => "Frost protection alarm counter: how long the alarm condition has lasted",
            15022 => "Defrosting alarm return status. 0=Returned, 1=Not returned",
            15028 => "Defrosting alarm counter: how long the alarm condition has lasted",
            15029 => "Supply air fan feedback alarm return status. 0=Returned, 1=Not returned",
            15035 => "Supply air fan feedback alarm counter: how long the alarm condition has lasted",
            15036 => "Extract air fan feedback alarm return status. 0=Returned, 1=Not returned",
            15042 => "Extract air fan feedback alarm counter: how long the alarm condition has lasted",
            15057 => "Frost protection temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15063 => "Frost protection temperature sensor alarm counter: how long the alarm condition has lasted",
            15064 => "Outside air temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15070 => "Outside air temperature sensor alarm counter: how long the alarm condition has lasted",
            15071 => "Supply air temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15077 => "Supply air temperature sensor alarm counter: how long the alarm condition has lasted",
            15078 => "Room air temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15084 => "Room air temperature sensor alarm counter: how long the alarm condition has lasted",
            15085 => "Extract air temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15091 => "Extract air temperature sensor alarm counter: how long the alarm condition has lasted",
            15092 => "Extra controller air temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15098 => "Extra controller air temperature sensor alarm counter: how long the alarm condition has lasted",
            15099 => "Efficiency air temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15105 => "Efficiency air temperature sensor alarm counter: how long the alarm condition has lasted",
            15106 => "Overheat temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15112 => "Overheat temperature sensor alarm counter: how long the alarm condition has lasted",
            15113 => "Emergency thermostat alarm return status. 0=Returned, 1=Not returned",
            15119 => "Emergency thermostat alarm counter: how long the alarm condition has lasted",
            15120 => "Rotary heat exchanger rotation guard alarm return status. 0=Returned, 1=Not returned",
            15126 => "Rotary heat exchanger rotation guard alarm counter: how long the alarm condition has lasted",
            15127 => "Plate heat exchanger bypass damper alarm return status. 0=Returned, 1=Not returned",
            15133 => "Plate heat exchanger bypass damper alarm counter: how long the alarm condition has lasted",
            15134 => "Secondary air damper alarm return status. 0=Returned, 1=Not returned",
            15140 => "Secondary air damper alarm counter: how long the alarm condition has lasted",
            15141 => "Filter alarm return status. 0=Returned, 1=Not returned",
            15147 => "Filter alarm counter: how long the alarm condition has lasted",
            15148 => "Extra controller alarm return status. 0=Returned, 1=Not returned",
            15154 => "Extra controller alarm counter: how long the alarm condition has lasted",
            15155 => "External stop alarm return status. 0=Returned, 1=Not returned",
            15161 => "External stop alarm counter: how long the alarm condition has lasted",
            15162 => "External relative humidity sensor alarm return status. 0=Returned, 1=Not returned",
            15168 => "External relative humidity sensor alarm counter: how long the alarm condition has lasted",
            15169 => "External CO₂ sensor alarm return status. 0=Returned, 1=Not returned",
            15175 => "External CO₂ sensor alarm counter: how long the alarm condition has lasted",
            15176 => "Low supply air temperature alarm return status. 0=Returned, 1=Not returned",
            15182 => "Low supply air temperature alarm counter: how long the alarm condition has lasted",
            15183 => "Plate heat exchanger bypass damper feedback alarm return status. 0=Returned, 1=Not returned",
            15189 => "Plate heat exchanger bypass damper feedback alarm counter: how long the alarm condition has lasted",
            15301 => "Timestamp of the supply air fan control alarm, lower 16 bits",
            15302 => "Timestamp of the supply air fan control alarm, higher 16 bits",
            15303 => "Timestamp of the extract air fan control alarm, lower 16 bits",
            15304 => "Timestamp of the extract air fan control alarm, higher 16 bits",
            15305 => "Timestamp of the frost protection alarm, lower 16 bits",
            15306 => "Timestamp of the frost protection alarm, higher 16 bits",
            15307 => "Timestamp of the defrosting alarm, lower 16 bits",
            15308 => "Timestamp of the defrosting alarm, higher 16 bits",
            15309 => "Timestamp of the supply air fan feedback alarm, lower 16 bits",
            15310 => "Timestamp of the supply air fan feedback alarm, higher 16 bits",
            15311 => "Timestamp of the extract air fan feedback alarm, lower 16 bits",
            15312 => "Timestamp of the extract air fan feedback alarm, higher 16 bits",
            15317 => "Timestamp of the frost protection temperature sensor alarm, lower 16 bits",
            15318 => "Timestamp of the frost protection temperature sensor alarm, higher 16 bits",
            15319 => "Timestamp of the outside air temperature sensor alarm, lower 16 bits",
            15320 => "Timestamp of the outside air temperature sensor alarm, higher 16 bits",
            15321 => "Timestamp of the supply air temperature sensor alarm, lower 16 bits",
            15322 => "Timestamp of the supply air temperature sensor alarm, higher 16 bits",
            15323 => "Timestamp of the room air temperature sensor alarm, lower 16 bits",
            15324 => "Timestamp of the room air temperature sensor alarm, higher 16 bits",
            15325 => "Timestamp of the extract air temperature sensor alarm, lower 16 bits",
            15326 => "Timestamp of the extract air temperature sensor alarm, higher 16 bits",
            15327 => "Timestamp of the extra controller air temperature sensor alarm, lower 16 bits",
            15328 => "Timestamp of the extra controller air temperature sensor alarm, higher 16 bits",
            15329 => "Timestamp of the efficiency air temperature sensor alarm, lower 16 bits",
            15330 => "Timestamp of the efficiency air temperature sensor alarm, higher 16 bits",
            15331 => "Timestamp of the overheat temperature sensor alarm, lower 16 bits",
            15332 => "Timestamp of the overheat temperature sensor alarm, higher 16 bits",
            15333 => "Timestamp of the emergency thermostat alarm, lower 16 bits",
            15334 => "Timestamp of the emergency thermostat alarm, higher 16 bits",
            15335 => "Timestamp of the rotary heat exchanger rotation guard alarm, lower 16 bits",
            15336 => "Timestamp of the rotary heat exchanger rotation guard alarm, higher 16 bits",
            15337 => "Timestamp of the plate heat exchanger bypass damper alarm, lower 16 bits",
            15338 => "Timestamp of the plate heat exchanger bypass damper alarm, higher 16 bits",
            15339 => "Timestamp of the secondary air damper alarm, lower 16 bits",
            15340 => "Timestamp of the secondary air damper alarm, higher 16 bits",
            15341 => "Timestamp of the filter alarm, lower 16 bits",
            15342 => "Timestamp of the filter alarm, higher 16 bits",
            15343 => "Timestamp of the extra controller alarm, lower 16 bits",
            15344 => "Timestamp of the extra controller alarm, higher 16 bits",
            15345 => "Timestamp of the external stop alarm, lower 16 bits",
            15346 => "Timestamp of the external stop alarm, higher 16 bits",
            15347 => "Timestamp of the external relative humidity sensor alarm, lower 16 bits",
            15348 => "Timestamp of the external relative humidity sensor alarm, higher 16 bits",
            15349 => "Timestamp of the external CO₂ sensor alarm, lower 16 bits",
            15350 => "Timestamp of the external CO₂ sensor alarm, higher 16 bits",
            15351 => "Timestamp of the low supply air temperature alarm, lower 16 bits",
            15352 => "Timestamp of the low supply air temperature alarm, higher 16 bits",
            15353 => "Timestamp of the fire alarm, lower 16 bits",
            15354 => "Timestamp of the fire alarm, higher 16 bits",
            15355 => "Timestamp of the plate heat exchanger bypass damper feedback alarm, lower 16 bits",
            15356 => "Timestamp of the plate heat exchanger bypass damper feedback alarm, higher 16 bits",
            15508 => "Internal relative humidity sensor alarm return status. 0=Returned, 1=Not returned",
            15514 => "Internal relative humidity sensor alarm counter: how long the alarm condition has lasted",
            15515 => "Internal extract air temperature sensor alarm return status. 0=Returned, 1=Not returned",
            15521 => "Internal extract air temperature sensor alarm counter: how long the alarm condition has lasted",
            15529 => "Heater overheat alarm return status. 0=Returned, 1=Not returned",
            15535 => "Heater overheat alarm counter: how long the alarm condition has lasted",
            15536 => "Fire alarm return status. 0=Returned, 1=Not returned",
            15542 => "Fire alarm counter: how long the alarm condition has lasted",
            15543 => "Filter warning alarm return status. 0=Returned, 1=Not returned",
            15603 => "Timestamp of the internal relative humidity sensor alarm, lower 16 bits",
            15604 => "Timestamp of the internal relative humidity sensor alarm, higher 16 bits",
            15605 => "Timestamp of the internal extract air temperature sensor alarm, lower 16 bits",
            15606 => "Timestamp of the internal extract air temperature sensor alarm, higher 16 bits",
            15609 => "Timestamp of the heater overheat alarm, lower 16 bits",
            15610 => "Timestamp of the heater overheat alarm, higher 16 bits",
            15611 => "Timestamp of the filter warning alarm, lower 16 bits",
            15612 => "Timestamp of the filter warning alarm, higher 16 bits",
            15905 => "Clears all active alarms. 1=Clear",
            15906 => "Number of active alarms",
            15907 => "Any alarm indication. 0=Not supported, 1=No active alarms, 2=Active alarm",
            15910 => "Low supply air temperature alarm monitoring. 0=Not monitored, 1=Monitored",
            2133 => "Heat exchanger type. 0=Rotating, 1=Plate",
            3002 => "Reheater type. 0=None, 1=Electrical, 2=Water, 3=Change-over",
            3003 => "Bypass damper location. 0=Supply, 1=Extract",
            3014 => "Cooler type. 0=None, 1=Water, 2=Change-over",
            3015 => "Extra controller function. 0=None, 1=Preheater, 2=Heating, 3=Cooling, 4=GEO",
            8951 => "Unit model. 2=VSR 300, others in the SAVE CONNECT enum",
            11221 | 11222 | 11223 | 11224 | 11225 | 11226 => "Correction added to the universal input's analog reading",
            11301 | 11302 | 11303 | 11304 | 11305 | 11306 => "Universal input digital function, same values as DI_CONNECTION_n",
            11321 | 11322 | 11323 | 11324 | 11325 | 11326 => "Polarity of the universal input in digital mode. 0=NO, 1=NC",
            11411 => "Fan feedback type",
            11501 => "Whether the built-in (PDM) RH sensor is connected",
            11502 => "Whether the built-in (PDM) temperature sensor is connected",
            11503 => "Correction added to the built-in (PDM) RH sensor's reading",
            11504 => "Correction added to the built-in (PDM) extract air temperature reading",
            13021 | 13022 | 13023 | 13024 => "Actuator type. 0=0–10 V, 1=2–10 V, 2=10–0 V, 3=10–2 V",
            15191 => "Heater malfunction alarm. 0=Inactive, 1=Active, 2=Waiting, 3=Acknowledged",
            1040 => "Reads 0 on firmware 1.15, probably unused or from older firmware. SAVE CONNECT names it both SUMMER_WINTER_T and CFG_STORAGE_TRANSFER_STATE",
            2111 => "Start of the heater's part of the supply air temperature controller output, on a 0–10 scale",
            2112 => "End of the heater's part of the supply air temperature controller output, on a 0–10 scale",
            2131 => "Start of the heat exchanger's part of the supply air temperature controller output, on a 0–10 scale",
            2132 => "End of the heat exchanger's part of the supply air temperature controller output, on a 0–10 scale",
            2301 => "Start of the cooler's part of the supply air temperature controller output, on a 0–10 scale",
            2302 => "End of the cooler's part of the supply air temperature controller output, on a 0–10 scale",
            8021 => "Unit model, an ASCII string over the following registers",
            8901 => "Serial number, an ASCII string over the following registers",
            13034 => "Temperature setpoint analog output (the setpoint scaled between CFG_AO_T_SP_LOW_VALUE and _HIGH_VALUE)",
            30004 | 30005 | 30006 | 30007 | 30008 | 30009 | 30010 => "Raw analog input voltage",
            30065 => "Main board uptime in seconds, lower 16 bits",
            30066 => "Main board uptime in seconds, higher 16 bits (inferred)",
            _ => "",
        };
        index += 1;
    }
    result
};
