from datetime import datetime

import numpy

# with open(sys.argv[2], "r") as f:
#     for line in f:
#         ts, type, remainder = line.split(" ", 2)
#         if type == "TIMESYNC":
#             remainder = remainder.strip()[1:-1]
#             tc1, ts1 = remainder.split(", ")
#             tc1 = int(tc1.split("tc1 : ")[1]) / 1e9
#             if tc1 == 0.0:
#                 continue
#             ts1 = int(ts1.split("ts1 : ")[1]) / 1e9
#             tc1_datetime = datetime.fromtimestamp(tc1, tz=timezone.utc)
#             print(tc1_datetime, ts1)

class TimeSync:
    def __init__(self, map: dict[float, float]):
        self._from = [v[0] for v in map]
        self._from_min_max = (min(self._from), max(self._from))

        self._to = [v[1] for v in map]
        self._to_min_max = (min(self._to), max(self._to))

    def forward(self, dt: datetime) -> datetime | None:
        ts = dt.timestamp()
        if ts < self._from_min_max[0] or ts > self._from_min_max[1]:
            return None
        result = numpy.interp(ts, self._from, self._to)
        return datetime.fromtimestamp(result)

    def reverse(self, dt: datetime) -> datetime | None:
        ts = dt.timestamp()
        if ts < self._to_min_max[0] or ts > self._to_min_max[1]:
            return None
        result = numpy.interp(ts, self._to, self._from)
        return datetime.fromtimestamp(result)

# test_ts_rpi = [1755721161.019, 1755721299.223, 1755721105.043]
# result_ts_fc = ts_rpi_to_fc(test_ts_rpi)
# print(f"{result_ts_fc[0]} {result_ts_fc[1]} {result_ts_fc[2]}")

# test_ts_fc = [1755721161.802059, 1755721299.999963, 1755721105.827077]
# result_ts_rpi = ts_fc_to_rpi(test_ts_fc)
# print(f"{result_ts_rpi[0]} {result_ts_rpi[1]} {result_ts_rpi[2]}")
