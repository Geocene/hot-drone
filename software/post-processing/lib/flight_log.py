from datetime import datetime, timedelta, timezone

from pymavlink import mavutil
import numpy

# Observed types in log from 2025/08/20 PM flight:
#
# 'AHR2', 'ARM', 'ATT', 'AUXF', 'BARO', 'BAT', 'CMD', 'CTRL', 'CTUN', 'D32', 'DCM',
# 'DSF', 'DU32', 'EAHR', 'ERR', 'EV', 'FILE', 'FMT', 'FMTU', 'GPA', 'GPS', 'HEAT',
# 'IMU', 'IOMC', 'MAG', 'MAV', 'MCU', 'MODE', 'MOTB', 'MSG', 'MULT', 'ORGN', 'PARM',
# 'PM', 'POS', 'POWR', 'PSCD', 'PSCE', 'PSCN', 'RAD', 'RATE', 'RCI2', 'RCIN', 'RCO2',
# 'RCOU', 'RSSI', 'SRTL', 'STAK', 'TERR', 'TSYN', 'UBX2', 'UNIT', 'VER', 'VIBE',
# 'XKF1', 'XKF2', 'XKF3', 'XKF4', 'XKF5', 'XKFS', 'XKQ', 'XKT', 'XKV1', 'XKV2',
# 'XKY0', 'XKY1'

class FlightLog:
    def __init__(self, log_path: str):
        self._gpa = []
        self._gps = []
        self._attitudes = []
        self._positions = []

        self._handlers = {
            'ATT': self._handle_att,
            # 'MSG': self._handle_msg,
            'GPA': self._handle_gpa,
            # 'GPS': self._handle_gps,
            'POS': self._handle_pos,
        }

        self._read(log_path)
        self._process()

    # ATT: Canonical vehicle attitude
    def _handle_att(self, ts, d):
        roll = d["Roll"]
        pitch = d["Pitch"]
        yaw = d["Yaw"]

        self._attitudes.append({
            "ts": ts,       # seconds, UNIX epoch, UTC
            "roll": roll,   # degrees
            "pitch": pitch, # degrees
            "yaw": yaw      # degrees heading
        })

    # GPA: GPS accuracy information
    def _handle_gpa(self, ts, d):
        self._gpa.append({
            "v_dop": d["VDop"],   # vertical dilution of precision
            "h_acc": d["HAcc"],   # horizontal position accuracy, meters
            "v_acc": d["VAcc"],   # vertical position accuracy, meters
            "s_acc": d["SAcc"],   # speed accuracy, m/s
            "y_acc": d["YAcc"],   # yaw accuracy, degrees
            # "aei": d["AEI"],      # altitude above WGS-84 ellipsoid; INT32_MIN (-2147483648) if unknown, meters
        })

    # GPS: Information received from GNSS systems attached to the autopilot
    def _handle_gps(self, ts, d):
        gms = d["GMS"]
        gwk = d["GWk"]

        # Produce datetime based on GPS epoch.
        gps_datum = datetime(1980, 1, 6, 0, 0, 0, tzinfo=timezone.utc)
        gps_week = gps_datum + timedelta(days=gwk * 7)
        gps_datetime = gps_week + timedelta(milliseconds=gms)

        # https://en.wikipedia.org/wiki/Leap_second
        leap_seconds = 18
        gps_datetime = gps_datetime + timedelta(seconds=-leap_seconds)

        # print(f"{ts} gps {gms} {gwk} -> {gps_datetime}")
        self._gps.append({
            "ts": ts,
            "gms": gms,
            "gwk": gwk,
            "h_dop": d["HDop"], # horizontal dilution of precision
            "ground_speed": d["Spd"],   # ground speed, meters/second
            "ground_course": d["GCrs"], # ground course, degrees heading
            "vertical_speed": d["VZ"],  # vertical speed, meters/second
            "yaw": d["Yaw"],            # vehicle yaw, degrees heading
        })

    # MSG: Textual messages
    def _handle_msg(self, ts, d):
        message_str = d["Message"]
        print(f"{ts} msg \"{message_str}\"")

    # POS: Canonical vehicle position
    def _handle_pos(self, ts, d):
        lat = d["Lat"]
        lng = d["Lng"]
        alt = d["Alt"]

        self._positions.append({
            "ts": ts,       # seconds, UNIX epoch, UTC
            "lat": lat,     # degrees
            "lng": lng,     # degrees
            "alt": alt,     # meters (MSL, it seems. Must reference from terrain data at lift-off?)
        })

    def _read(self, log_path: str):
        log = mavutil.mavlink_connection(log_path, notimestamps=False, robust_parsing=False)

        while True:
            message = log.recv_match()
            if message is None:
                break

            handler = self._handlers.get(message.get_type(), None)
            if handler is not None:
                ts_local = getattr(message, "_timestamp", None)
                # ts_local = datetime.fromtimestamp(ts_local, tz=TZ_LOCAL)

                d = message.to_dict()
                handler(ts_local, d)

    def _process(self):
        self._attitudes_ts = numpy.array([v["ts"   ] for v in self._attitudes], dtype=numpy.float64)
        self._rolls        = numpy.array([v["roll" ] for v in self._attitudes], dtype=numpy.float64)
        self._pitches      = numpy.array([v["pitch"] for v in self._attitudes], dtype=numpy.float64)
        self._yaws         = numpy.array([v["yaw"  ] for v in self._attitudes], dtype=numpy.float64)

        self._positions_ts = numpy.array([v["ts" ] for v in self._positions], dtype=numpy.float64)
        self._latitudes    = numpy.array([v["lat"] for v in self._positions], dtype=numpy.float64)
        self._longitudes   = numpy.array([v["lng"] for v in self._positions], dtype=numpy.float64)
        self._altitudes    = numpy.array([v["alt"] for v in self._positions], dtype=numpy.float64)

        # print("attitudes", min(self._attitudes_ts), max(self._attitudes_ts))
        # print("positions", min(self._positions_ts), max(self._positions_ts))

        # assert(numpy.all(self._positions_ts == sorted(self._positions_ts)))

        # print(self._positions_ts)

    # NOTE: attitudes are assumed sorted by increasing timestamp.

    @property
    def attitudes_ts(self): # -> [float]:
        return self._attitudes_ts

    @property
    def rolls(self): # -> list[float]:
        return self._rolls

    @property
    def pitches(self): # -> list[float]:
        return self._pitches

    @property
    def yaws(self): # -> list[float]:
        return self._yaws

    @property
    def positions_ts(self): # -> list[float]:
        return self._positions_ts

    @property
    def latitudes(self): # -> list[float]:
        return self._latitudes

    @property
    def longitudes(self): # -> list[float]:
        return self._longitudes

    @property
    def altitudes(self): # -> list[float]:
        return self._altitudes

    def attitude_interp(self, dt: datetime):
        # TODO: Warning! These are angles, they wrap! Interpolation
        # will potentially make a mess of these values if two adjacent
        # values have wrapped.
        ts = dt.timestamp()
        return {
            "roll":  numpy.interp(ts, self._attitudes_ts, self._rolls),
            "pitch": numpy.interp(ts, self._attitudes_ts, self._pitches),
            "yaw":   numpy.interp(ts, self._attitudes_ts, self._yaws),
        }

    def position_interp(self, dt: datetime):
        ts = dt.timestamp()
        return {
            "lat": numpy.interp(ts, self._positions_ts, self._latitudes),
            "lng": numpy.interp(ts, self._positions_ts, self._longitudes),
            "alt": numpy.interp(ts, self._positions_ts, self._altitudes),
        }

# # RAD: Telemetry radio statistics
# rssi_local = d["RSSI"]
# rssi_remote = d["RemRSSI"]
# noise_local = d["Noise"]
# noise_remote = d["RemNoise"]
# print(f"{boot_ts} rad {rssi_local}:{rssi_remote} {noise_local}:{noise_remote}")


# # ARM:
# arm_state = d['ArmState']
# if arm_state:
#     print(f"{boot_ts} armed")
# else:
#     print("{boot_ts} disarmed")

# # BARO: Gathered barometer data
# alt = d["Alt"] # Altitude above launch point.
# # AltAMSL may not be present if AMSL pressure not set?
# # alt_amsl = d["AltAMSL"]
# print(f"{boot_ts} baro {alt}")

# # CMD:
# c_num = d["CNum"]
# c_id = d["CId"]
# lat = d["Lat"]
# lng = d["Lng"]
# alt = d["Alt"]
# print(f"{boot_ts} cmd {c_num} {c_id} {lat} {lng} {alt}")

# # DCM:
# wind_n_bound = d["VWN"]
# wind_e_bound = d["VWE"]
# wind_d_bound = d["VWD"]
# print("f{boot_ts} dcm {wind_n_bound} {wind_e_bound} {wind_d_bound}")

# # EAHR: External AHRS data
# pass

# # GPA: GPS accuracy information
# pass

# # ORGN: Vehicle navigation origin or other notable position
# type =d["Type"]
# lat = d["Lat"]
# lng = d["Lng"]
# alt = d["Alt"]
# print(f"{boot_ts} orgn {type} {lat} {lng} {alt}")
