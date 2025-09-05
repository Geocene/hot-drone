import json
import os
import os.path
import re
from collections import defaultdict

import numpy

from lib.path import Flight, MetadataFile

class Metadata:
    def __init__(self, flight_path: str, camera_names: dict[int, str]):
        self._d = self._read_dir(flight_path, camera_names)

    def _read_dir(self, flight: Flight, camera_names: dict[int, str]) -> dict[list]:
        result = defaultdict(list[MetadataFile])
        for file in flight.metadata_files:
            file.data["camera_name"] = camera_names[file.cam]
            result[file.ts].append(file)

        return result

    @property
    def timestamps(self) -> list[float]:
        return numpy.array(sorted([v["ts"] for v in self._d]), dtype=numpy.float32)
