#[derive(Clone, Copy, serde::Serialize, PartialEq, Eq)]
pub struct DataType {
    scale: u8,
    signed: bool,
}

impl DataType {
    // Convenience aliases for nicely tabulated `for_each_regsiter` macro definition below.
    pub const U16: Self = Self {
        scale: 1,
        signed: false,
    };
    pub const I16: Self = Self {
        scale: 1,
        signed: true,
    };
    pub const CEL: Self = Self {
        scale: 10,
        signed: true,
    };
    pub const SPH: Self = Self {
        scale: 10,
        signed: false,
    };

    pub fn from_bytes<'a>(self, mut bs: &'a [u8]) -> impl Iterator<Item = Value> + 'a {
        std::iter::from_fn(move || {
            let (v, remainder) = bs.split_first_chunk::<2>()?;
            bs = remainder;
            Some(match self {
                Self::I16 => Value::I16(i16::from_be_bytes(*v)),
                Self::U16 => Value::U16(u16::from_be_bytes(*v)),
                Self::CEL => Value::Celsius(i16::from_be_bytes(*v)),
                Self::SPH => Value::SpecificHumidity(u16::from_be_bytes(*v)),
                _ => panic!("malformed DataType"),
            })
        })
    }

    pub const fn is_signed(&self) -> bool {
        self.signed
    }
    pub const fn scale(&self) -> u8 {
        self.scale
    }

    pub const fn bytes(&self) -> usize {
        2
    }
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.signed { "S/" } else { "U/" })?;
        f.write_fmt(format_args!("{}", self.scale))?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum Value {
    U16(u16),
    I16(i16),
    /// This data type contains a value multiplied by 10.
    Celsius(i16),
    /// This data type contains a value multiplied by 10.
    SpecificHumidity(u16),
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
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Value::U16(n) => f.write_fmt(format_args!("{}", n)),
            Value::I16(n) => f.write_fmt(format_args!("{}", n)),
            Value::Celsius(n) => f.write_fmt(format_args!("{}", n as f32 / 10.0)),
            Value::SpecificHumidity(n) => f.write_fmt(format_args!("{}", n as f32 / 10.0)),
        }
    }
}

impl serde::Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Value::U16(n) => serializer.serialize_u16(n),
            Value::I16(n) => serializer.serialize_i16(n),
            Value::Celsius(n) => serializer.serialize_f32(n as f32 / 10.0),
            Value::SpecificHumidity(n) => serializer.serialize_f32(n as f32 / 10.0),
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
}

#[derive(Clone, Copy)]
pub struct RegisterIndex(usize);

impl RegisterIndex {
    pub fn from_address(address: u16) -> Option<RegisterIndex> {
        let index = ADDRESSES.partition_point(|v| *v < address);
        (ADDRESSES[index] == address).then_some(Self(index))
    }

    pub fn from_name(name: &str) -> Option<RegisterIndex> {
        let index = NAMES.into_iter().position(|v| *v == name);
        index.map(Self)
    }

    pub fn address(&self) -> u16 {
        ADDRESSES[self.0]
    }

    pub fn name(&self) -> &'static str {
        NAMES[self.0]
    }

    pub fn data_type(&self) -> DataType {
        DATA_TYPES[self.0]
    }
}


macro_rules! for_each_register {
    ($m:ident) => {
        $m! {
            1001: U16, R_, "DEMC_RH_HIGHEST", min = 0, max = 100;
            1002: U16, R_, "DEMC_CO2_HIGHEST", min = 0, max = 2000;
            1011: U16, R_, "DEMC_RH_PI_SP", min = 0, max = 100;
            1012: U16, R_, "DEMC_RH_PI_FEEDBACK", min = 0, max = 100;
            1019: U16, R_, "DEMC_RH_PI_OUTPUT";
            1021: U16, R_, "DEMC_CO2_PI_SP", min = 0, max = 2000;
            1022: U16, R_, "DEMC_CO2_PI_FEEDBACK", min = 0, max = 2000;
            1029: U16, R_, "DEMC_CO2_PI_OUTPUT";
            1031: U16, RW, "DEMC_RH_SETTINGS_PBAND", min = 1, max = 100;
            1033: U16, RW, "DEMC_RH_SETTINGS_SP_SUMMER", min = 10, max = 100;
            1034: U16, RW, "DEMC_RH_SETTINGS_SP_WINTER", min = 10, max = 100;
            1035: U16, RW, "DEMC_RH_SETTINGS_ON_OFF", min = 0, max = 1;
            1039: U16, R_, "SUMMER_WINTER", min = 0, max = 1;
            1041: U16, RW, "DEMC_CO2_SETTINGS_PBAND", min = 50, max = 2000;
            1043: U16, RW, "DEMC_CO2_SETTINGS_SP", min = 100, max = 2000;
            1044: U16, RW, "DEMC_CO2_SETTINGS_ON_OFF", min = 0, max = 1;
            1101: U16, RW, "USERMODE_HOLIDAY_TIME", min = 1, max = 365;
            1102: U16, RW, "USERMODE_AWAY_TIME", min = 1, max = 72;
            1103: U16, RW, "USERMODE_FIREPLACE_TIME", min = 1, max = 60;
            1104: U16, RW, "USERMODE_REFRESH_TIME", min = 1, max = 240;
            1105: U16, RW, "USERMODE_CR_WDED_TIME", min = 1, max = 8;
            1111: U16, R_, "USERMODE_REMAINING_TIME_L";
            1112: U16, R_, "USERMODE_REMAINING_TIME_H";
            1121: U16, RW, "IAQ_SPEED_LEVEL_MIN", min = 2, max = 3;
            1122: U16, RW, "IAQ_SPEED_LEVEL_MAX", min = 3, max = 5;
            1123: U16, R_, "IAQ_LEVEL", min = 0, max = 2;
            1131: U16, RW, "USERMODE_MANUAL_AIRFLOW_LEVEL_SAF", min = 0, max = 4;
            1135: U16, RW, "USERMODE_CR_WDED_AIRFLOW_LEVEL_SAF", min = 3, max = 5;
            1136: U16, RW, "USERMODE_CR_WDED_AIRFLOW_LEVEL_EAF", min = 3, max = 5;
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
            1151: CEL, RW, "USERMODE_CR_WDED_T_OFFSET", min = -100, max = 0;
            1161: U16, R_, "USERMODE_MODE", min = 0, max = 12;
            1162: U16, RW, "USERMODE_HMI_CHANGE_REQUEST", min = 0, max = 7;
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
            1183: U16, RW, "USERMODE_FIRPLACE_DI_OFF_DELAY", min = 0, max = 60;
            1184: U16, RW, "USERMODE_REFRESH_DI_OFF_DELAY", min = 0, max = 240;
            1185: U16, RW, "USERMODE_CR_WDED_DI_OFF_DELAY", min = 0, max = 8;
            1188: U16, RW, "CDI1_OFF_DELAY", min = 0, max = 240;
            1189: U16, RW, "CDI2_OFF_DELAY", min = 0, max = 240;
            1190: U16, RW, "CDI3_OFF_DELAY", min = 0, max = 240;
            1221: U16, R_, "SPEED_CDI1_SAF";
            1222: U16, R_, "SPEED_CDI1_EAF";
            1223: U16, R_, "SPEED_CDI2_SAF";
            1224: U16, R_, "SPEED_CDI2_EAF";
            1225: U16, R_, "SPEED_CDI3_SAF";
            1226: U16, R_, "SPEED_CDI3_EAF";
            1227: U16, R_, "SPEED_PRESSURE_GUARD_SAF";
            1228: U16, R_, "SPEED_PRESSURE_GUARD_EAF";
            1251: U16, RW, "FAN_OUTDOOR_COMP_TYPE", min = 0, max = 1;
            1252: CEL, RW, "FAN_OUTDOOR_COMP_MAX_VALUE", min = 0, max = 50;
            1253: CEL, RW, "FAN_OUTDOOR_COMP_STOP_T_WINTER", min = -300, max = 0;
            1254: CEL, RW, "FAN_OUTDOOR_COMP_MAX_TEMP", min = -300, max = 0;
            1255: U16, R_, "FAN_OUTDOOR_COMP_RESULT", min = 0, max = 100;
            1256: CEL, RW, "FAN_OUTDOOR_COMP_START_T_WINTER", min = -300, max = 0;
            1257: CEL, RW, "FAN_OUTDOOR_COMP_START_T_SUMMER", min = 150, max = 300;
            1258: CEL, RW, "FAN_OUTDOOR_COMP_STOP_T_SUMMER", min = 150, max = 400;
            1259: CEL, RW, "FAN_OUTDOOR_COMP_VALUE_SUMMER", min = 0, max = 50;
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
            1357: U16, R_, "SPEED_ELECTRICAL_HEATER_HOT_COUNTER";
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
            1411: U16, RW, "FAN_LEVEL_SAF_MIN_RPM", min = 500, max = 5000;
            1412: U16, RW, "FAN_LEVEL_EAF_MIN_RPM", min = 500, max = 5000;
            1413: U16, RW, "FAN_LEVEL_SAF_LOW_RPM", min = 500, max = 5000;
            1414: U16, RW, "FAN_LEVEL_EAF_LOW_RPM", min = 500, max = 5000;
            1415: U16, RW, "FAN_LEVEL_SAF_NORMAL_RPM", min = 500, max = 5000;
            1416: U16, RW, "FAN_LEVEL_EAF_NORMAL_RPM", min = 500, max = 5000;
            1417: U16, RW, "FAN_LEVEL_SAF_HIGH_RPM", min = 500, max = 5000;
            1418: U16, RW, "FAN_LEVEL_EAF_HIGH_RPM", min = 500, max = 5000;
            1419: U16, RW, "FAN_LEVEL_SAF_MAX_RPM", min = 500, max = 5000;
            1420: U16, RW, "FAN_LEVEL_EAF_MAX_RPM", min = 500, max = 5000;
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
            1621: U16, R_, "USERMODE_REMAINING_TIME_CDI1_L";
            1622: U16, R_, "USERMODE_REMAINING_TIME_CDI1_H";
            1623: U16, R_, "USERMODE_REMAINING_TIME_CDI2_L";
            1624: U16, R_, "USERMODE_REMAINING_TIME_CDI2_H";
            1625: U16, R_, "USERMODE_REMAINING_TIME_CDI3_L";
            1626: U16, R_, "USERMODE_REMAINING_TIME_CDI3_H";
            2001: CEL, RW, "TC_SP", min = 120, max = 300;
            2013: CEL, RW, "TC_CASCADE_SP", min = 120, max = 400;
            2021: CEL, RW, "TC_CASCADE_SP_MIN", min = 120, max = 400;
            2022: CEL, RW, "TC_CASCADE_SP_MAX", min = 120, max = 400;
            2031: U16, RW, "TC_CONTR_L_MODE", min = 0, max = 2;
            2051: CEL, R_, "TC_EAT_RAT_SP", min = 120, max = 400;
            2053: CEL, R_, "TC_ROOM_CTRL_SP_SATC", min = 120, max = 400;
            2054: CEL, R_, "TC_SP_SATC", min = 120, max = 300;
            2055: U16, R_, "SATC_HEAT_DEMAND", min = 0, max = 100;
            2061: CEL, R_, "SATC_PI_SP", min = 120, max = 300;
            2069: I16, R_, "SATC_PI_OUTPUT", min = 0, max = 100;
            2071: CEL, R_, "ROOM_CTRL_PI_SP", min = 120, max = 300;
            2079: I16, R_, "ROOM_CTRL_PI_OUTPUT", min = 0, max = 100;
            2101: U16, R_, "INPUT_EXTERNAL_CTRL_SAF", min = 0, max = 100;
            2102: U16, R_, "INPUT_EXTERNAL_CTRL_EAF", min = 0, max = 100;
            2113: CEL, RW, "HEATER_CIRC_PUMP_START_T", min = 0, max = 200;
            2122: U16, RW, "HEATER_CIRC_PUMP_STOP_DELAY", min = 0, max = 60;
            2134: U16, RW, "HEAT_EXCHANGER_COOLING_RECOVERY_ON_OFF", min = 0, max = 1;
            2147: U16, R_, "HEAT_EXCHANGER_RH_TRANSFER_CTRL_ENABLED";
            2148: U16, R_, "HEAT_EXCHANGER_SPEED_LIMIT_RH_TRANSFER", min = 0, max = 100;
            2149: U16, R_, "PWM_TRIAC_OUTPUT", min = 0, max = 100;
            2201: U16, RW, "R_TOR_RH_TRANSFER_CTRL_PBAND", min = 0, max = 40;
            2202: U16, RW, "R_TOR_RH_TRANSFER_CTRL_ITIME", min = 120, max = 0;
            2203: U16, RW, "R_TOR_RH_TRANSFER_CTRL_SETPOINT", min = 100, max = 45;
            2204: U16, RW, "R_TOR_RH_TRANSFER_CTRL_ON_OFF", min = 1, max = 1;
            2211: SPH, R_, "ROTOR_EA_SPEC_HUMIDITY", min = 0;
            2212: SPH, R_, "ROTOR_OA_SPEC_HUMIDITY", min = 0;
            2213: SPH, R_, "ROTOR_EA_SPEC_HUMIDITY_SETPOINT", min = 0;
            2311: U16, R_, "COOLER_FROM_SATC", min = 0, max = 100;
            2314: CEL, RW, "COOLER_CIRC_PUMP_START_T", min = 0, max = 200;
            2315: CEL, RW, "COOLER_RECOVERY_LIMIT_T", min = 0, max = 100;
            2316: CEL, RW, "COOLER_OAT_INTERLOCK_T", min = 120, max = 250;
            2317: U16, RW, "COOLER_CIRC_PUMP_STOP_DELAY", min = 0, max = 60;
            2403: CEL, RW, "EXTRA_CONTR_LLER_SET_PI_SETPOINT", min = -300, max = 400;
            2404: CEL, RW, "EXTRA_CONTR_LLER_CIRC_PUMP_START_T", min = 0, max = 200;
            2405: U16, RW, "EXTRA_CONTR_LLER_CIRC_PUMP_STOP_DELAY", min = 0, max = 60;
            2418: U16, RW, "EXTRA_CONTR_LLER_PREHEATER_SETPOINT_TYPE", min = 0, max = 1;
            2420: CEL, RW, "EXTRA_CONTR_LLER_GEO_PREHEATER_SP", min = -300, max = 100;
            2421: CEL, RW, "EXTRA_CONTR_LLER_GEO_PREHEATER_ACTIVATION_T", min = -300, max = 0;
            2422: CEL, RW, "EXTRA_CONTR_LLER_GEO_PRECOOLER_SP", min = 100, max = 300;
            2423: CEL, RW, "EXTRA_CONTR_LLER_GEO_PRECOOLER_ACTIVATION_T", min = 150, max = 300;
            2451: CEL, RW, "CHANGE_OVER_CIRC_PUMP_START_T", min = 0, max = 200;
            2452: U16, RW, "CHANGE_OVER_CIRC_PUMP_STOP_DELAY", min = 0, max = 60;
            2504: CEL, RW, "ECO_T_Y1_OFFSET", min = 0, max = 100;
            2505: U16, RW, "ECO_MODE_ON_OFF", min = 0, max = 1;
            2506: U16, R_, "ECO_FUNCTION_ACTIVE", min = 0, max = 1;
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
            4101: U16, RW, "FREE_COOLING_ON_OFF", min = 0, max = 1;
            4102: CEL, RW, "FREE_COOLING_OUTDOOR_DAYTIME_T", min = 120, max = 300;
            4103: CEL, RW, "FREE_COOLING_OUTDOOR_NIGHTTIME_DEACTIVATION_HIGH_T_LIMIT", min = 70, max = 300;
            4104: CEL, RW, "FREE_COOLING_OUTDOOR_NIGHTTIME_DEACTIVATION_LOW_T_LIMIT", min = 70, max = 300;
            4105: CEL, RW, "FREE_COOLING_R_OM_CANCEL_T", min = 120, max = 300;
            4106: U16, RW, "FREE_COOLING_START_TIME_H";
            4107: U16, RW, "FREE_COOLING_START_TIME_M", min = 0, max = 59;
            4108: U16, RW, "FREE_COOLING_END_TIME_H";
            4109: U16, RW, "FREE_COOLING_END_TIME_M", min = 0, max = 59;
            4111: U16, R_, "FREE_COOLING_ACTIVE", min = 0, max = 1;
            4112: U16, RW, "FREE_COOLING_MIN_SPEED_LEVEL_SAF", min = 3, max = 5;
            4113: U16, RW, "FREE_COOLING_MIN_SPEED_LEVEL_EAF", min = 3, max = 5;
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
            9001: U16, RW, "SYSTEM_UNIT_FLOW", min = 0, max = 2;
            9002: U16, RW, "SYSTEM_UNIT_PRESSURE", min = 0, max = 1;
            9003: U16, RW, "SYSTEM_UNIT_TEMPERATURE", min = 0, max = 1;
            11401: U16, RW, "DI_CONNECTION_1", min = 0, max = 18;
            11402: U16, RW, "DI_CONNECTION_2", min = 0, max = 18;
            11421: U16, RW, "DI_CFG_POLARITY_1", min = 0, max = 1;
            11422: U16, RW, "DI_CFG_POLARITY_2", min = 0, max = 1;
            12011: U16, RW, "INPUT_ANALOG_UI_1";
            12012: U16, RW, "INPUT_ANALOG_UI_2";
            12013: U16, RW, "INPUT_ANALOG_UI_3";
            12014: U16, RW, "INPUT_ANALOG_UI_4";
            12015: U16, RW, "INPUT_ANALOG_UI_5";
            12016: U16, R_, "INPUT_ANALOG_UI_6";
            12021: U16, RW, "INPUT_DIGITAL_UI_1", min = 0, max = 1;
            12022: U16, RW, "INPUT_DIGITAL_UI_2", min = 0, max = 1;
            12023: U16, RW, "INPUT_DIGITAL_UI_3", min = 0, max = 1;
            12024: U16, RW, "INPUT_DIGITAL_UI_4", min = 0, max = 1;
            12025: U16, RW, "INPUT_DIGITAL_UI_5", min = 0, max = 1;
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
            12112: U16, R_, "SENSOR_RGS", min = 0, max = 1;
            12115: U16, RW, "SENSOR_CO2S", min = 0, max = 2000;
            12136: U16, RW, "SENSOR_RHS_PDM", min = 0, max = 100;
            12151: U16, RW, "SENSOR_CO2S_1", min = 0, max = 2000;
            12152: U16, RW, "SENSOR_CO2S_2", min = 0, max = 2000;
            12153: U16, RW, "SENSOR_CO2S_3", min = 0, max = 2000;
            12154: U16, RW, "SENSOR_CO2S_4", min = 0, max = 2000;
            12155: U16, RW, "SENSOR_CO2S_5", min = 0, max = 2000;
            12156: U16, R_, "SENSOR_CO2S_6";
            12161: U16, RW, "SENSOR_RHS_1", min = 0, max = 100;
            12162: U16, RW, "SENSOR_RHS_2", min = 0, max = 100;
            12163: U16, RW, "SENSOR_RHS_3", min = 0, max = 100;
            12164: U16, RW, "SENSOR_RHS_4", min = 0, max = 100;
            12165: U16, RW, "SENSOR_RHS_5", min = 0, max = 100;
            12166: U16, R_, "SENSOR_RHS_6", min = 0, max = 100;
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
            12401: U16, R_, "SENSOR_RPM_SAF", min = 0, max = 5000;
            12402: U16, R_, "SENSOR_RPM_EAF", min = 0, max = 5000;
            12403: U16, R_, "SENSOR_FLOW_PIGGYBACK_SAF";
            12404: U16, R_, "SENSOR_FLOW_PIGGYBACK_EAF";
            12405: U16, R_, "SENSOR_DI_BYF";
            12544: CEL, RW, "SENSOR_PDM_EAT_VALUE", min = -400, max = 800;
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
            12983: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_OAT_VALUE", min = -410, max = 810;
            12984: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_SAT_VALUE", min = -410, max = 810;
            12985: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_OHT_VALUE", min = -410, max = 810;
            12986: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_FPT_VALUE", min = -410, max = 810;
            12987: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_RAT_VALUE", min = -410, max = 810;
            12988: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_EAT_VALUE", min = -410, max = 810;
            12989: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_ECT_VALUE", min = -410, max = 810;
            12990: CEL, RW, "MANUAL_OVERRIDE_F_INPUT_EFT_VALUE", min = -410, max = 810;
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
            13601: U16, RW, "MANUAL_OVERRIDE_OUTPUT_SAF", min = 0, max = 1;
            13602: U16, RW, "MANUAL_OVERRIDE_OUTPUT_EAF", min = 0, max = 1;
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
            14351: U16, R_, "OUTPUT_AO1";
            14352: U16, R_, "OUTPUT_AO2";
            14353: U16, R_, "OUTPUT_AO3";
            14354: U16, R_, "OUTPUT_AO4";
            14355: U16, R_, "OUTPUT_AO5";
            14361: U16, R_, "OUTPUT_DO1", min = 0, max = 1;
            14362: U16, R_, "OUTPUT_DO2", min = 0, max = 1;
            14363: U16, R_, "OUTPUT_DO3", min = 0, max = 1;
            14364: U16, R_, "OUTPUT_DO4", min = 0, max = 1;
            14371: U16, R_, "OUTPUT_FAN_SPEED1", min = 0, max = 100;
            14372: U16, R_, "OUTPUT_FAN_SPEED2", min = 0, max = 100;
            14381: U16, R_, "OUTPUT_TRIAC", min = 0, max = 1;
            15002: U16, R_, "ALARM_SAF_CTRL_ALARM", min = 0, max = 3;
            15003: U16, RW, "ALARM_SAF_CTRL_CLEAR_ALARM", min = 0, max = 1;
            15009: U16, R_, "ALARM_EAF_CTRL_ALARM", min = 0, max = 3;
            15010: U16, RW, "ALARM_EAF_CTRL_CLEAR_ALARM", min = 0, max = 1;
            15016: U16, R_, "ALARM_FROST_PROT_ALARM", min = 0, max = 3;
            15017: U16, RW, "ALARM_FR_ST_PROT_CLEAR_ALARM", min = 0, max = 1;
            15023: U16, R_, "ALARM_DEFROSTING_ALARM", min = 0, max = 3;
            15024: U16, RW, "ALARM_DEFR_STING_CLEAR_ALARM", min = 0, max = 1;
            15030: U16, R_, "ALARM_SAF_RPM_ALARM", min = 0, max = 3;
            15031: U16, RW, "ALARM_SAF_RPM_CLEAR_ALARM", min = 0, max = 1;
            15037: U16, R_, "ALARM_EAF_RPM_ALARM", min = 0, max = 3;
            15038: U16, RW, "ALARM_EAF_RPM_CLEAR_ALARM", min = 0, max = 1;
            15058: U16, R_, "ALARM_FPT_ALARM", min = 0, max = 3;
            15059: U16, RW, "ALARM_FPT_CLEAR_ALARM", min = 0, max = 1;
            15065: U16, R_, "ALARM_OAT_ALARM", min = 0, max = 3;
            15066: U16, RW, "ALARM_OAT_CLEAR_ALARM", min = 0, max = 1;
            15072: U16, R_, "ALARM_SAT_ALARM", min = 0, max = 3;
            15073: U16, RW, "ALARM_SAT_CLEAR_ALARM", min = 0, max = 1;
            15079: U16, R_, "ALARM_RAT_ALARM", min = 0, max = 3;
            15080: U16, RW, "ALARM_RAT_CLEAR_ALARM", min = 0, max = 1;
            15086: U16, R_, "ALARM_EAT_ALARM", min = 0, max = 3;
            15087: U16, RW, "ALARM_EAT_CLEAR_ALARM", min = 0, max = 1;
            15093: U16, R_, "ALARM_ECT_ALARM", min = 0, max = 3;
            15094: U16, RW, "ALARM_ECT_CLEAR_ALARM", min = 0, max = 1;
            15100: U16, R_, "ALARM_EFT_ALARM", min = 0, max = 3;
            15101: U16, RW, "ALARM_EFT_CLEAR_ALARM", min = 0, max = 1;
            15107: U16, R_, "ALARM_OHT_ALARM", min = 0, max = 3;
            15108: U16, RW, "ALARM_OHT_CLEAR_ALARM", min = 0, max = 1;
            15114: U16, R_, "ALARM_EMT_ALARM", min = 0, max = 3;
            15115: U16, RW, "ALARM_EMT_CLEAR_ALARM", min = 0, max = 1;
            15121: U16, R_, "ALARM_RGS_ALARM", min = 0, max = 3;
            15122: U16, RW, "ALARM_RGS_CLEAR_ALARM", min = 0, max = 1;
            15128: U16, R_, "ALARM_BYS_ALARM", min = 0, max = 3;
            15129: U16, RW, "ALARM_BYS_CLEAR_ALARM", min = 0, max = 1;
            15135: U16, R_, "ALARM_SECONDARY_AIR_ALARM", min = 0, max = 3;
            15136: U16, RW, "ALARM_SECONDARY_AIR_CLEAR_ALARM", min = 0, max = 1;
            15142: U16, R_, "ALARM_FILTER_ALARM", min = 0, max = 3;
            15143: U16, RW, "ALARM_FILTER_CLEAR_ALARM", min = 0, max = 1;
            15149: U16, R_, "ALARM_EXTRA_CONTROLLER_ALARM", min = 0, max = 3;
            15150: U16, RW, "ALARM_EXTRA_CONTR_LLER_CLEAR_ALARM", min = 0, max = 1;
            15156: U16, R_, "ALARM_EXTERNAL_STOP_ALARM", min = 0, max = 3;
            15157: U16, RW, "ALARM_EXTERNAL_STOP_CLEAR_ALARM", min = 0, max = 1;
            15163: U16, R_, "ALARM_RH_ALARM", min = 0, max = 3;
            15164: U16, RW, "ALARM_RH_CLEAR_ALARM", min = 0, max = 1;
            15170: U16, R_, "ALARM_CO2_ALARM", min = 0, max = 3;
            15171: U16, RW, "ALARM_CO2_CLEAR_ALARM", min = 0, max = 1;
            15177: U16, R_, "ALARM_LOW_SAT_ALARM", min = 0, max = 3;
            15178: U16, RW, "ALARM_LOW_SAT_CLEAR_ALARM", min = 0, max = 1;
            15184: U16, R_, "ALARM_BYF_ALARM";
            15185: U16, RW, "ALARM_BYF_CLEAR_ALARM", max = 0;
            15502: U16, R_, "ALARM_MANUAL_OVERRIDE_OUTPUTS_ALARM", min = 0, max = 3;
            15503: U16, RW, "ALARM_MANUAL_OVERRIDE_OUTPUTS_CLEAR_ALARM", min = 0, max = 1;
            15509: U16, R_, "ALARM_PDM_RHS_ALARM", min = 0, max = 3;
            15510: U16, RW, "ALARM_PDM_RHS_CLEAR_ALARM", min = 0, max = 1;
            15516: U16, R_, "ALARM_PDM_EAT_ALARM", min = 0, max = 3;
            15517: U16, RW, "ALARM_PDM_EAT_CLEAR_ALARM", min = 0, max = 1;
            15523: U16, R_, "ALARM_MANUAL_FAN_STOP_ALARM", min = 0, max = 3;
            15524: U16, RW, "ALARM_MANUAL_FAN_STOP_CLEAR_ALARM", min = 0, max = 1;
            15530: U16, R_, "ALARM_OVERHEAT_TEMPERATURE_ALARM", min = 0, max = 3;
            15531: U16, RW, "ALARM_OVERHEAT_TEMPERATURE_CLEAR_ALARM", min = 0, max = 1;
            15537: U16, R_, "ALARM_FIRE_ALARM_ALARM", min = 0, max = 3;
            15538: U16, RW, "ALARM_FIRE_ALARM_CLEAR_ALARM", min = 0, max = 1;
            15544: U16, R_, "ALARM_FILTER_WARNING_ALARM", min = 0, max = 3;
            15545: U16, RW, "ALARM_FILTER_WARNING_CLEAR_ALARM", min = 0, max = 1;
            15549: U16, R_, "ALARM_FILTER_WARNING_ALARM_ERROR_DURATION_COUNTER";
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
            30101: U16, RW, "FACTORY_RESET", min = 3228, max = 3228;
            30103: U16, RW, "SET_USER_SAFE_CONFIG", min = 0, max = 1;
            30104: U16, RW, "ACTIVATE_USER_SAFE_CONFIG", min = 0, max = 1;
            30105: U16, R_, "USER_SAFE_CONFIG_VALID";
            30106: U16, R_, "SAFE_CONFIG_VALID";
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
            1019 => {
                "Output value for RH demand control. (1): Depends on regulation type. Value can be \
                 %, RPM, Pressure or Flow"
            }
            1021 => "Set point for CO2 demand control",
            1022 => "Sensor value for CO2 demand control",
            1029 => {
                "Output value for CO2 demand control. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1031 => "Pband setting for RH demand control",
            1033 => "Set point setting for RH demand control winter time",
            1034 => "Set point setting for RH demand control summer time",
            1035 => "Flag indicating if RH demand control is allowed",
            1039 => "Actual seasson for Demand Control. 0=Summer, 1=Winter",
            1041 => "Pband setting for CO2 demand control",
            1043 => "Set point setting for CO2 demand control",
            1044 => "Flag indicating if CO2 demand control isallowed",
            1101 => "Time delay setting for user mode Holiday",
            1102 => "Time delay setting for user mode Away",
            1103 => "Time delay setting for user mode Fire Place",
            1104 => "Time delay setting for user mode Refresh",
            1105 => "Time delay setting for user mode Crowded",
            1111 | 1112 => "Remaining time for the state Holiday/Away/Fire Place/Refresh/Crowded",
            1121 => "Minimum level for Demand Control. 2=Low, 3=Normal",
            1122 => "Maximum level for user Demand Control. 3=Normal, 4=High, 5=Maximum",
            1123 => "Actual IAQ level. 0=Economic, 1=Good, 2=Improving",
            1131 => {
                "Fan speed level for mode Manual. Applies to both the SAF and the EAF fan. 0=Off, \
                 2=Low, 3=Normal, 4=High. Value Off only allowed if contents of register \
                 REG_FAN_MANUAL_STOP_ALLOWED is 1."
            }
            1135 | 1136 => "Fan speed level for mode Crowded. 3=Normal, 4=High, 5=Maximum",
            1137 | 1138 => "Fan speed level for mode Refresh. 3=Normal, 4=High, 5=Maximum",
            1139 | 1140 => "Fan speed level for mode Fire Place. 1=Minimum, 2=Low, 3=Normal",
            1141 | 1142 => {
                "Fan speed level for mode Away. Value Off only allowed if contents of register \
                 REG_FAN_MANUAL_STOP_ALLOWED is 1."
            }
            1143 | 1144 => {
                "Fan speed level for mode Holiday. Value Off only allowed if contents of register \
                 REG_FAN_MANUAL_STOP_ALLOWED is 1."
            }
            1145 | 1146 => "Fan speed level for mode Cooker Hood. 1=Minimum, 2=Low, 3=Normal",
            1147 | 1148 => "Fan speed level for mode Vacuum Cleaner. 1=Minimum, 2=Low, 3=Normal",
            1151 => "Temperature setpoint offset for user mode Crowded",
            1161 => {
                "Active User mode. 0=Auto, 1=Manual, 2=Crowded, 3=Refresh, 4=Fireplace, 5=Away, \
                 6=Holiday, 7=Cooker Hood, 8=Vacuum Cleaner, 9=CDI1, 10=CDI2, 11=CDI3, \
                 12=PressureGuard"
            }
            1162 => {
                "New desired user mode as requested by HMI. 0=None, 1=AUTO, 2=Manual, 3=Crowded, \
                 4=Refresh, 5=Fireplace, 6=Away, 7=Holiday"
            }
            1171 | 1172 | 1173 | 1174 | 1175 | 1176 => {
                "Fan speed level for configurable digital input 3. 0=Off, 1=Minimum, 2=Low, \
                 3=Normal, 4=High, 5=Maximum"
            }
            1177 | 1178 => {
                "Fan speed level for configurable pressure guard function. 0=Off, 1=Minimum, \
                 2=Low, 3=Normal, 4=High, 5=Maximum"
            }
            1181 | 1182 | 1183 | 1184 | 1185 | 1188 | 1189 | 1190 => "Off delay for DI",
            1221 => {
                "SAF speed value for user mode Holiday. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1222 => {
                "EAF speed value for user mode Holiday. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1223 => {
                "SAF speed value for mode Cooker Hood. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1224 => {
                "EAF speed value for mode Cooker Hood. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1225 => {
                "SAF speed value for mode Vacuum Cleaner. (1): Depends on regulation type. Value \
                 can be %, RPM, Pressure or Flow"
            }
            1226 => {
                "EAF speed value for mode Vacuum Cleaner. (1): Depends on regulation type. Value \
                 can be %, RPM, Pressure or Flow"
            }
            1227 | 1303 => {
                "SAF speed value for low fan speed. (1): Depends on regulation type. Value can be \
                 %, RPM, Pressure or Flow"
            }
            1228 | 1304 => {
                "EAF speed value for low fan speed. (1): Depends on regulation type. Value can be \
                 %, RPM, Pressure or Flow"
            }
            1251 => "Compensate only SF or both SF and EF. 0=SAF, 1=SAF/EAF",
            1252 => "Compensation value at lowest temperature.",
            1253 => {
                "Temperature at which compensation reaches maximum value during the winter period."
            }
            1254 => "Temperature at which highest compensation is applied.",
            1255 => "Current outdoor compensation value",
            1256 => "Temperature at which compensation starts during the winter period.",
            1257 => "Temperature at which compensation starts during the summer period.",
            1258 => {
                "Temperature at which compensation reaches maximum value during the summer period."
            }
            1259 => "Compensation value during summer period",
            1274 => {
                "Type of fan control mode. 0=Manual, 1=RPM, 2=VAV (Constant Pressure), 3=CAV \
                 (Constant Flow), 4=DCV (External)"
            }
            1301 => {
                "SAF speed value for minimum fan speed. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1302 => {
                "EAF speed value for minimum fan speed. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1305 => {
                "SAF speed value for normal fan speed. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1306 => {
                "EAF speed value for normal fan speed. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1307 => {
                "SAF speed value for high fan speed. (1): Depends on regulation type. Value can be \
                 %, RPM, Pressure or Flow"
            }
            1308 => {
                "EAF speed value for high fan speed. (1): Depends on regulation type. Value can be \
                 %, RPM, Pressure or Flow"
            }
            1309 => {
                "SAF speed value for maximum fan speed. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1310 => {
                "EAF speed value for maximum fan speed. (1): Depends on regulation type. Value can \
                 be %, RPM, Pressure or Flow"
            }
            1351 => "Indicates that both fans are running",
            1352 => {
                "Indicates that the SAF shall be turned off once the electrical reheater is cooled \
                 down."
            }
            1353 => {
                "Allow manual fan stop (also as selection for user modes and Week schedule). \
                 0=Manual stop not allowed, 1=Manual stop allowed"
            }
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
            2001 | 2054 => "Temperature setpoint for the supply airtemperature",
            2013 | 2053 => {
                "Temperature set point for SATC, as calculated by RATC/EATC during cascade control"
            }
            2021 => "Minimum temperature set point for the SATC",
            2022 => "Maximum temperature set point for the SATC",
            2031 => "Unit temperature control mode. 0=Supply, 1=Room, 2=Extract",
            2051 => "EAT or RAT value, used for room/extract air controller.",
            2055 => "Output of the SATC (0-100%)",
            2061 => "SATC setpoint value",
            2069 => "SATC output signal",
            2071 => "RATC setpoint value",
            2079 => "RATC output signal",
            2101 => "Value from External Controller Input, SAF. In %.",
            2102 => "Value from External Controller Input, EAF. In %.",
            2113 => "Temperature at which the Heater circulation pump is started",
            2122 => "Off time delay for the heater circulation pump in minutes",
            2134 => "Enabling of cooling recovery",
            2147 => "Indicates if RH trasnfer control shall beapplied",
            2148 => {
                "As the heat exchanger unit is rotates faster, more moisture returns indoors. If \
                 there is a demand to eject moisture outdoors, the rotor speed is limited. This \
                 register determines the maximum speed of the rotor due to moisture transfer."
            }
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
            3101 | 3102 | 3103 | 3104 | 3105 | 3106 | 3107 | 3108 | 3109 | 3110 | 3111 | 3112
            | 3113 | 3114 | 3115 | 3116 | 3117 => "Is the function currently active?",
            4101 => "Indicates if free cooling is enabled",
            4102 => "Minimum of highest daytime temperature for start of free cooling.",
            4103 => "Highest night temperature limit for termination free cooling",
            4104 => "Lowest night temperature limit for termination free cooling",
            4105 => "Lowest temperature room temperature for termination of free cooling",
            4106 => {
                "Start time of free cooling night-period, hour. Valid range is from 0 to 8 and \
                 from 21 to 23."
            }
            4107 => "Start time of free cooling night-period, Minute",
            4108 => {
                "End time of free cooling night-period, hour. Valid range is from 0 to 8 and from \
                 21 to 23."
            }
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
            5060 => {
                "Fan speed levels for SAF and EAF during active week schedule. 1=Off, 2=Low, \
                 3=Normal, 4=High, 5=Demand. Off available if Manual Fan Stop is enabled. Demand \
                 available if demand control active or external fan control enabled."
            }
            5061 => {
                "Fan speed levels for SAF and EAF during inactive week schedule. 1=Off, 2=Low, \
                 3=Normal, 4=High, 5=Demand. Off available if Manual Fan Stop is enabled. Demand \
                 available if demand control active or external fan control enabled."
            }
            5101 | 5102 | 5103 | 5104 | 5105 | 5106 | 5107 | 5108 | 5109 | 5110 | 5111 | 5112
            | 5113 | 5114 => "Flag indicating if this period is enabled.",
            6001 | 6002 | 6003 | 6004 | 6005 | 6006 => "Current time",
            6007 => {
                "Flag indicating if DST is enabled. 0=Daylight saving time not enabled, 1=Daylight \
                 saving time enabled"
            }
            6008 => "Indicaties the presentation of time in the HMI. 24H/12H",
            6009 => "Monday (0)...Sunday (6)",
            6011 => "Now time in seconds. Lower 16 bits.",
            6012 => "Now time in seconds. Higher 16 bits.",
            6101 => "RTC value in seconds, highest 16 bits",
            7001 => "Filter replacement time in months",
            7002 => "Timestamp of latest filter replcement, lower 16 bits",
            7003 => "Timestamp of latest filter replcement, higher 16 bits",
            7004 => "Indicates that the LastFilterReplacementTime shall be set Now.",
            7005 => "Remaining filter time in seconds, lower 16 bits.",
            7006 => "Remaining filter time in seconds, higher 16 bits.",
            7007 => "Indicates if the filter warning alarm wasgenerated.",
            9001 => "Unit for CAV control mode. 0=l/s, 1=m³/h, 2=cfm",
            9002 => "Units for VAV control mode. 0=Pa, 1=InH2O",
            9003 => "Units for temperature. 0=Celcius, 1=Fahrenheit",
            11401 | 11402 => {
                "Indicates what kind of DI functionality is connected to DI1. 0=None, 1=Away, \
                 2=BYP, 3=Vacuum Cleaner, 4=Cooker Hood, 5=Crowded, 6=EMT, 7=External Stop, \
                 8=Extra Controller Alarm, 9=Fireplace, 10=Holiday, 11=Refresh, 12=RGS, 13=Change \
                 Over Feedback, 14=Fire Alarm, 15=Configurable DI1, 16=Configurable DI2, \
                 17=Configurable DI3, 18=Pressure Guard"
            }
            11421 | 11422 => "Polarity of DI1. 0=NO, 1=NC",
            12011 | 12012 | 12013 | 12014 | 12015 | 12016 => "mV",
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
            12405 => "Value from Bypass Damper Feedback input.In %.",
            12544 => "PDM EAT sensor value (standard)",
            12931 | 12932 | 12933 | 12934 | 12935 | 12936 | 12937 | 12938 | 12939 | 12940
            | 12941 | 12942 | 12943 | 12944 | 12945 | 12946 | 12947 | 12948 | 12949 | 12950 => {
                "Enable manual override of the device input. 0=AUTO, 1=OVERRIDE"
            }
            12951 | 12952 | 12953 | 12954 | 12955 | 12956 | 12957 | 12958 | 12959 | 12960
            | 12961 | 12962 | 12963 | 12964 | 12983 | 12984 | 12985 | 12986 | 12987 | 12988
            | 12989 | 12990 => "Value to override the device input with.",
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
            14104 => "Heat Exchanger DO state. 0=Output notactive, 1=Output active",
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
            14371 => "Supply air fan control signal in %",
            14372 => "Extract air fan control signal in %",
            14381 => "TRIAC control signal",
            15002 | 15009 | 15016 | 15023 | 15030 | 15037 | 15058 | 15065 | 15072 | 15079
            | 15086 | 15093 | 15100 | 15107 | 15114 | 15121 | 15128 | 15135 | 15142 | 15149
            | 15156 | 15163 | 15170 | 15177 | 15502 | 15509 | 15516 | 15523 | 15530 | 15537
            | 15544 | 15184 => {
                "Alarm active/inactive. 0=Inactive, 1=Active, 2=Waiting, 3=Cleared Error Active"
            }
            15003 | 15010 | 15017 | 15024 | 15031 | 15038 | 15059 | 15066 | 15073 | 15080
            | 15087 | 15094 | 15101 | 15108 | 15115 | 15122 | 15129 | 15136 | 15143 | 15150
            | 15157 | 15164 | 15171 | 15178 | 15185 | 15503 | 15510 | 15517 | 15524 | 15531
            | 15538 | 15545 => "Signal to clear the alarm",
            15549 => "Counter for delay",
            15701 | 15702 | 15703 | 15704 | 15705 | 15706 | 15707 | 15708 | 15709 | 15710
            | 15711 | 15712 | 15713 | 15714 | 15715 | 15716 | 15717 | 15718 | 15719 | 15720
            | 15721 | 15722 | 15723 | 15724 | 15725 | 15726 | 15727 | 15728 | 15729 | 15730
            | 15731 | 15732 | 15733 | 15734 | 15735 | 15736 | 15737 | 15738 | 15739 | 15740
            | 15741 | 15742 | 15743 | 15744 | 15745 | 15746 | 15747 | 15748 | 15749 | 15750
            | 15751 | 15752 | 15753 | 15754 | 15755 | 15756 | 15757 | 15758 | 15759 | 15760
            | 15761 | 15762 | 15763 | 15764 | 15765 | 15766 | 15767 | 15768 | 15769 | 15770
            | 15771 | 15772 | 15773 | 15774 | 15775 | 15776 | 15777 | 15778 | 15779 | 15780
            | 15781 | 15782 | 15783 | 15784 | 15785 | 15786 | 15787 | 15788 | 15789 | 15790
            | 15791 | 15792 | 15793 | 15794 | 15795 | 15796 | 15797 | 15798 | 15799 | 15800
            | 15801 | 15802 | 15803 | 15804 | 15805 | 15806 | 15807 | 15808 | 15809 | 15810
            | 15811 | 15812 | 15813 | 15814 | 15815 | 15816 | 15817 | 15818 | 15819 | 15820
            | 15821 | 15822 | 15823 | 15824 | 15825 | 15826 | 15827 | 15828 | 15829 | 15830
            | 15831 | 15832 | 15833 | 15834 | 15835 | 15836 | 15837 | 15838 | 15839 | 15840
            | 15841 | 15842 | 15843 | 15844 | 15845 | 15846 | 15847 | 15848 | 15849 | 15850
            | 15851 | 15852 | 15853 | 15854 | 15855 | 15856 | 15857 | 15858 | 15859 | 15860
            | 15861 | 15862 | 15863 | 15864 | 15865 | 15866 | 15867 | 15868 | 15869 | 15870
            | 15871 | 15872 | 15873 | 15874 | 15875 | 15876 | 15877 | 15878 | 15879 | 15880
            | 15881 | 15882 | 15883 | 15884 | 15885 | 15886 | 15887 | 15888 | 15889 | 15890
            | 15891 | 15892 | 15893 | 15894 | 15895 | 15896 | 15897 | 15898 | 15899 | 15900 => {
                "Alarm log information"
            }
            15901 => "Indicates if an alarm Type A is active",
            15902 => "Indicates if an alarm Type B is active",
            15903 => "Indicates if an alarm Type C is active",
            16001 => {
                "Administrator password. Bit 12-15: digit 1, Bit 8-11: digit 2, Bit 4-7: digit 3, \
                 Bit 0-3: digit 4"
            }
            16002 => {
                "Indicates if the User level is locked. 0=User menu locked, 1=User menu not locked"
            }
            16003 => "Indicates if the Filter menu is locked. 0=menu locked, 1=menu not locked",
            16004 => {
                "Indicates if the Week schedule menu islocked. 0=menu locked, 1=menu not locked"
            }
            16051 => "Home screen lock",
            16052 => "Filter Change menu lock",
            16053 => "Week schedule menu lock",
            16101 => "Indicates if the start-up wizard shall beactivated.",
            17001 => "Modbus address of the MB. Only relevant if the MB is a modbus slave.",
            17002 => {
                "Baudrate of the modbus connection. 0=1200, 1=2400, 2=4800, 3=9600, 4=14400, \
                 5=19200, 6=28800, 7=38400, 8=57600, 9=76800, 10=115200"
            }
            17003 => "Parity setting for the modbus connection. 0=None, 1=Even, 2=Odd",
            30101 => {
                "Activates setting of the parameters to their default values. Only activated by \
                 writing 3228 to this register."
            }
            _ => "",
        };
        index += 1;
    }
    result
};

/*
class VirtualAlarmsDataType implements DataType<Record<string, number>> {
    read_commands(_1: RegisterDescription<Record<string, number>>): { address: number; count: number; }[] {
        return [
            { address: 14003, count: 1 }, // SUM alarm
            { address: 15000, count: 94 },
            { address: 15100, count: 85 },
            { address: 15500, count: 45 },
        ];
    }
    extract(buffers: Buffer[]): Record<string, number> {
        const [sum, a15000, a15100, a15500] = buffers;
        return {
            sum_alarm: sum.readUInt16BE(0),
            saf_ctrl_alarm: a15000.readUInt16BE(2 * 2),
            eaf_ctrl_alarm: a15000.readUInt16BE(9 * 2),
            frost_prot_alarm: a15000.readUInt16BE(16 * 2),
            defrosting_alarm: a15000.readUInt16BE(23 * 2),
            saf_rpm_alarm: a15000.readUInt16BE(30 * 2),
            eaf_rpm_alarm: a15000.readUInt16BE(37 * 2),
            fpt_alarm: a15000.readUInt16BE(58 * 2),
            oat_alarm: a15000.readUInt16BE(65 * 2),
            sat_alarm: a15000.readUInt16BE(72 * 2),
            rat_alarm: a15000.readUInt16BE(79 * 2),
            eat_alarm: a15000.readUInt16BE(86 * 2),
            ect_alarm: a15000.readUInt16BE(93 * 2),
            eft_alarm: a15100.readUInt16BE(0 * 2),
            oht_alarm: a15100.readUInt16BE(7 * 2),
            emt_alarm: a15100.readUInt16BE(14 * 2),
            rgs_alarm: a15100.readUInt16BE(21 * 2),
            bys_alarm: a15100.readUInt16BE(28 * 2),
            secondary_air_alarm: a15100.readUInt16BE(35 * 2),
            filter_alarm: a15100.readUInt16BE(42 * 2),
            extra_controller_alarm: a15100.readUInt16BE(49 * 2),
            external_stop_alarm: a15100.readUInt16BE(56 * 2),
            rh_alarm: a15100.readUInt16BE(63 * 2),
            co2_alarm: a15100.readUInt16BE(70 * 2),
            low_sat_alarm: a15100.readUInt16BE(77 * 2),
            byf_alarm: a15100.readUInt16BE(84 * 2),
            manual_override_outputs_alarm: a15500.readUInt16BE(2 * 2),
            pdm_rhs_alarm: a15500.readUInt16BE(9 * 2),
            pdm_eat_alarm: a15500.readUInt16BE(16 * 2),
            manual_fan_stop_alarm: a15500.readUInt16BE(23 * 2),
            overheat_temperature_alarm: a15500.readUInt16BE(30 * 2),
            fire_alarm_alarm: a15500.readUInt16BE(37 * 2),
            filter_warning_alarm: a15500.readUInt16BE(44 * 2),
        };
    }
    encode_writes(_1: RegisterDescription<Record<string, number>>, _2: Record<string, number>): never {
        /// FIXME: writing this could clear the alarm states...
        throw new Error("ALARM virtual register is not writable (yet)!");
    }
}

class VirtualActiveFunctionDataType implements DataType<Record<string, number>> {
    read_commands(_1: RegisterDescription<Record<string, number>>): { address: number; count: number; }[] {
        return [
            { address: 2506, count: 1 },
            { address: 3100, count: 18 },
            { address: 4111, count: 1 },
        ];
    }
    extract(buffers: Buffer[]): Record<string, number> {
        const [eco, fa, fca] = buffers;
        return {
            eco_function_active: eco.readUInt16BE(0),
            free_cooling_active: fca.readUInt16BE(0),
            function_active_cooling: fa.readUInt16BE(2 * 1),
            function_active_free_cooling: fa.readUInt16BE(2 * 2),
            function_active_heating: fa.readUInt16BE(2 * 3),
            function_active_defrosting: fa.readUInt16BE(2 * 4),
            function_active_heat_recovery: fa.readUInt16BE(2 * 5),
            function_active_cooling_recovery: fa.readUInt16BE(2 * 6),
            function_active_moisture_transfer: fa.readUInt16BE(2 * 7),
            function_active_secondary_air: fa.readUInt16BE(2 * 8),
            function_active_vacuum_cleaner: fa.readUInt16BE(2 * 9),
            function_active_cooker_hood: fa.readUInt16BE(2 * 10),
            function_active_user_lock: fa.readUInt16BE(2 * 11),
            function_active_eco_mode: fa.readUInt16BE(2 * 12),
            function_active_heater_cool_down: fa.readUInt16BE(2 * 13),
            function_active_pressure_guard: fa.readUInt16BE(2 * 14),
            function_active_cdi_1: fa.readUInt16BE(2 * 15),
            function_active_cdi_2: fa.readUInt16BE(2 * 16),
            function_active_cdi_3: fa.readUInt16BE(2 * 17),
        };
    }
    encode_writes(_1: RegisterDescription<Record<string, number>>, _2: Record<string, number>): never {
        throw new Error("ACTIVE_FUNCTIONS virtual register is not writable!");
    }
}

class VirtualSensorsDataType implements DataType<Record<string, number>> {
    read_commands(_1: RegisterDescription<Record<string, number>>): { address: number; count: number; }[] {
        return [
            { address: 12100, count: 67 },
            { address: 12400, count: 6 },
            { address: 12544, count: 1 },
        ];
    }
    extract(buffers: Buffer[]): Record<string, number> {
        const [s12100, s12400, pdm_eat] = buffers;
        return {
            fpt: s12100.readInt16BE(1 * 2) / 10,
            oat: s12100.readInt16BE(2 * 2) / 10,
            sat: s12100.readInt16BE(3 * 2) / 10,
            rat: s12100.readInt16BE(4 * 2) / 10,
            eat: s12100.readInt16BE(5 * 2) / 10,
            ect: s12100.readInt16BE(6 * 2) / 10,
            eft: s12100.readInt16BE(7 * 2) / 10,
            oht: s12100.readInt16BE(8 * 2) / 10,
            rhs: s12100.readUInt16BE(9 * 2),
            rgs: s12100.readUInt16BE(12 * 2),
            co2s: s12100.readUInt16BE(15 * 2),
            rhs_pdm: s12100.readUInt16BE(36 * 2),
            co2s_1: s12100.readUInt16BE(51 * 2),
            co2s_2: s12100.readUInt16BE(52 * 2),
            co2s_3: s12100.readUInt16BE(53 * 2),
            co2s_4: s12100.readUInt16BE(54 * 2),
            co2s_5: s12100.readUInt16BE(55 * 2),
            co2s_6: s12100.readUInt16BE(56 * 2),
            rhs_1: s12100.readUInt16BE(61 * 2),
            rhs_2: s12100.readUInt16BE(62 * 2),
            rhs_3: s12100.readUInt16BE(63 * 2),
            rhs_4: s12100.readUInt16BE(64 * 2),
            rhs_5: s12100.readUInt16BE(65 * 2),
            rhs_6: s12100.readUInt16BE(66 * 2),
            rpm_saf: s12400.readUInt16BE(1 * 2),
            rpm_eaf: s12400.readUInt16BE(2 * 2),
            flow_piggyback_saf: s12400.readUInt16BE(3 * 2),
            flow_piggyback_eaf: s12400.readUInt16BE(4 * 2),
            di_byf: s12400.readUInt16BE(5 * 2),
            pdm_eat_value: pdm_eat.readInt16BE(0) / 10,
        };
    }
    encode_writes(_1: RegisterDescription<Record<string, number>>, _2: Record<string, number>): never {
        throw new Error("SENSORS virtual register is not writable!");
    }
}

const virtual_registers = new Map<number, RegisterDescription<any>>([
    r(3100, new VirtualActiveFunctionDataType(), RO, "FUNCTION_ACTIVE", 0, 1),
    r(12100, new VirtualSensorsDataType(), RO, "SENSORS", 0, 1),
    r(15000, new VirtualAlarmsDataType(), RO, "ALARMS", 0, 3),
]);

function vd(register: number, description: string) {
    return d(register, description, virtual_registers);
}

vd(3100, "Read FUNCTION_ACTIVE_* registers as well as ECO_FUNCTION_ACTIVE and FREE_COOLING_ACTIVE.");
vd(12100, "Sensor states from registers 121xx, 124xx, and 12544 in a convenient JS record.");
vd(15000, "All alarm states from registers 14003, 150xx, 151xx and 155xx in a convenient JS record.");

export { registers, virtual_registers };
*/
