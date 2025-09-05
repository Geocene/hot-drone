from datetime import datetime
import json
import glob
import os.path
import re

from lib.flight_log import FlightLog

FILESPEC_IMAGE_RAW = "*_c?.srggb16"
RE_FILENAME_IMAGE_RAW = r"(?P<ts>\d+\.\d+)_c(?P<cam>\d+).srggb16"

FILESPEC_METADATA = "*_c?.json"
RE_FILENAME_METADATA = r"(?P<ts>\d+\.\d+)_c(?P<cam>\d+).json"

class RawFile:
    def __init__(self, path: str):
        _dir_path_raw, file_name = os.path.split(path)
        file_name_base, file_name_ext = os.path.splitext(file_name)
        match = re.fullmatch(RE_FILENAME_IMAGE_RAW, file_name)
        self._ts = float(match["ts"])
        self._cam = int(match["cam"])
        self._path = path
        self._file_name = file_name
        self._file_name_base = file_name_base

    @property
    def ts(self) -> float:
        return self._ts

    @property
    def datetime(self) -> datetime:
        return datetime.fromtimestamp(self.ts)

    @property
    def cam(self) -> int:
        return self._cam

    @property
    def path(self) -> str:
        return self._path

    @property
    def file_name(self) -> str:
        return self._file_name

    @property
    def file_name_base(self) -> str:
        return self._file_name_base

class MetadataFile:
    def __init__(self, path: str):
        _dir_path_raw, file_name = os.path.split(path)
        file_name_base, file_name_ext = os.path.splitext(file_name)
        match = re.fullmatch(RE_FILENAME_METADATA, file_name)
        self._ts = float(match["ts"])
        self._cam = int(match["cam"])
        self._path = path
        self._file_name = file_name
        self._file_name_base = file_name_base
        with open(path, "r") as f:
            self._data = json.load(f)

    @property
    def ts(self) -> float:
        return self._ts

    @property
    def datetime(self) -> datetime:
        return datetime.fromtimestamp(self.ts)

    @property
    def cam(self) -> int:
        return self._cam

    @property
    def path(self) -> str:
        return self._path

    @property
    def file_name(self) -> str:
        return self._file_name

    @property
    def file_name_base(self) -> str:
        return self._file_name_base

    @property
    def data(self) -> dict:
        return self._data

class Flight:
    def __init__(self, path_flight: str, file_name_log: str):
        self._path = path_flight

        path_log = os.path.join(path_flight, file_name_log)
        self._log = FlightLog(path_log)

    def metadata_for_raw_file(self, raw_file: RawFile) -> MetadataFile:
        path = os.path.join(self.path_meta, raw_file.file_name_base + ".json")
        return MetadataFile(path)

    def raw_file_for_metadata(self, metadata_file: MetadataFile) -> RawFile:
        path = os.path.join(self.path_raw, metadata_file.file_name_base + ".srggb16")
        return RawFile(path)

    @property
    def log(self) -> FlightLog:
        return self._log

    @property
    def path_meta(self) -> str:
        return os.path.join(self._path, "meta")

    @property
    def path_raw(self) -> str:
        return os.path.join(self._path, "raw")

    @property
    def raw_files(self) -> list[RawFile]:
        filespec = os.path.join(self.path_raw, FILESPEC_IMAGE_RAW)
        files = glob.glob(filespec)
        return [RawFile(path) for path in sorted(files)]

    @property
    def metadata_files(self) -> list[MetadataFile]:
        filespec = os.path.join(self.path_meta, FILESPEC_METADATA)
        files = glob.glob(filespec)
        return [MetadataFile(path) for path in sorted(files)]
