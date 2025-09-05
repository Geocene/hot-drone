from datetime import datetime
import re

import numpy

from pidng.core import RAW2DNG, DNGTags, Tag
from pidng.defs import *
from pidng.dng import Type

from .metadata import MetadataFile

class ExtraTag:
    GPSLatitudeRef  = (0x0001, Type.Ascii)
    GPSLatitude     = (0x0002, Type.Rational)
    GPSLongitudeRef = (0x0003, Type.Ascii)
    GPSLongitude    = (0x0004, Type.Rational)
    GPSAltitudeRef  = (0x0005, Type.Ascii)
    GPSAltitude     = (0x0006, Type.Rational)

def write_dng(ts: datetime, cam: int, data: numpy.ndarray, format: str, file_path: str, metadata_file: MetadataFile):
    height, stride = data.shape

    fmt_str = format.split("_")[0]
    bpp = int(re.search(r'\d+', fmt_str).group())

    data >>= (16 - bpp)

    # print(f"using bpp: {bpp}")

    metadata = metadata_file.data

    black_levels = list()
    for val in metadata.get("SensorBlackLevels", (0)):
        black_levels.append((val >> (16 - bpp)))
    # print(f"black levels: {black_levels}")

    camera_calibration = [[1, 1], [0, 1], [0, 1],
                          [0, 1], [1, 1], [0, 1],
                          [0, 1], [0, 1], [1, 1]]

    color_gain_div = 10000
    gain_r, gain_b = metadata.get("ColourGains",(1.0, 1.0))
    gain_matrix = numpy.array([[gain_r, 0.0, 0.0   ],
                               [0.0,    1.0, 0.0   ],
                               [0.0,    0.0, gain_b]])
    gain_r = int(gain_r * color_gain_div)
    gain_b = int(gain_b * color_gain_div)
    as_shot_neutral = [[color_gain_div, gain_r], [color_gain_div, color_gain_div], [color_gain_div, gain_b]]
    # print(f"as shot neutral: {as_shot_neutral}")

    ccm1 = list()
    ccm = metadata.get("ColourCorrectionMatrix", (1, 0, 0, 0, 1, 0, 0, 0, 1))
    # This maxtrix from http://www.brucelindbloom.com/index.html?Eqn_RGB_XYZ_Matrix.html
    rgb_to_xyz = numpy.array([[0.4124564, 0.3575761, 0.1804375],
                              [0.2126729, 0.7151522, 0.0721750],
                              [0.0193339, 0.1191920, 0.9503041]])
    ccm_matrix = numpy.array(ccm).reshape((3, 3))
    ccm = numpy.linalg.inv(rgb_to_xyz.dot(ccm_matrix).dot(gain_matrix))

    for color in ccm.flatten().tolist():
        ccm1.append((int(color*color_gain_div), color_gain_div))

    ci1 = CalibrationIlluminant.D65

    baseline_exp = 1

    make = "Sony"
    model = "IMX477"

    # profile_name = "PiDNG / PiCamera2 Profile"
    # profile_embed = 3

    # TODO: Use correct orientation value per camera.
    # TODO: Or just sort it out at the end of processing. That might be simpler. Otherwise, you risk double-rotation (or more!).
    orientation = Orientation.Horizontal
    # if cam == 2:
    #     orientation = 8

    cfaPattern = None
    if "BGGR" in fmt_str:
        cfaPattern = CFAPattern.BGGR
    elif "GBRG" in fmt_str:
        cfaPattern = CFAPattern.GBRG
    elif "GRBG" in fmt_str:
        cfaPattern = CFAPattern.GRBG
    elif "RGGB" in fmt_str:
        cfaPattern = CFAPattern.RGGB

    total_gain = metadata["AnalogueGain"] * metadata["DigitalGain"]
    # iso = int(round(total_gain * 100))

    sensor_image_area = (6.287, 4.712)
    sensor_pixel_size = (4056, 3040)

    focal_plane_x_resolution = [int(round(sensor_pixel_size[0] * 10 * 1000)), int(round(sensor_image_area[0] * 1000)) ]
    focal_plane_y_resolution = [int(round(sensor_pixel_size[1] * 10 * 1000)), int(round(sensor_image_area[1] * 1000)) ]

    # f_number = 2.8
    # f_number = [int(round(f_number * 10)), 10]

    # focal_length = 3.9 # mm
    # focal_length = [int(round(focal_length * 10)), 10]

    exposure_time_us = metadata["ExposureTime"]
    exposure_time_s = exposure_time_us * 1e-6
    exposure_time_s_fraction = 1 / exposure_time_s
    exposure_time_rational = [1, int(round(exposure_time_s_fraction))]
    # print(f"exposure time: 1/{exposure_time_s_fraction} == {1/exposure_time_s_fraction}")

    date_time_str = ts.strftime("%Y:%m:%d %H:%M:%S")
    date_time_subsec_str = ts.strftime("%f")[:3]

    tags = DNGTags()
    tags.set(Tag.DateTime, date_time_str)
    tags.set(Tag.SubsecTime, date_time_subsec_str)
    tags.set(Tag.DateTimeOriginal, date_time_str)
    tags.set(Tag.SubsecTimeOriginal, date_time_subsec_str)

    # tags.set(ExtraTag.GPSLatitudeRef,  "N")
    # tags.set(ExtraTag.GPSLatitude, )
    # tags.set(ExtraTag.GPSLongitudeRef, "W")
    # tags.set(ExtraTag.GPSLongitude, )
    # tags.set(ExtraTag.GPSAltitudeRef, )
    # tags.set(ExtraTag.GPSAltitude, )

    # tags.set(Tag.PhotographicSensitivity, [iso])
    # tags.set(Tag.ExposureTime, [exposure_time_rational])
    # tags.set(Tag.FNumber, [f_number])
    # tags.set(Tag.FocalLength, [focal_length])
    tags.set(Tag.RawDataUniqueID, f"{metadata['SensorTimestamp']:014d}c{cam:1d}".encode("ascii"))
    tags.set(Tag.ImageWidth, stride)
    tags.set(Tag.ImageLength, height)
    tags.set(Tag.Orientation, orientation)
    tags.set(Tag.SamplesPerPixel, 1)
    tags.set(Tag.BitsPerSample, bpp)
    tags.set(Tag.WhiteLevel, (1 << bpp) - 1 )
    tags.set(Tag.BaselineExposure, [[baseline_exp, 1]])
    # tags.set(Tag.BaselineExposure, [[-150, 100]])
    tags.set(Tag.Make, make)
    tags.set(Tag.Model, model)
    tags.set(Tag.FocalPlaneXResolution, [focal_plane_x_resolution])
    tags.set(Tag.FocalPlaneYResolution, [focal_plane_y_resolution])
    # tags.set(Tag.DateTimeOriginal)
    # tags.set(Tag.ProfileName, profile_name)
    # tags.set(Tag.ProfileEmbedPolicy, [profile_embed])

    # For colour Bayer sensors
    tags.set(Tag.BlackLevelRepeatDim, [2,2])
    tags.set(Tag.BlackLevel, black_levels)
    tags.set(Tag.PhotometricInterpretation, PhotometricInterpretation.Color_Filter_Array)
    tags.set(Tag.CFARepeatPatternDim, [2,2])
    tags.set(Tag.CFAPattern, cfaPattern)
    tags.set(Tag.ColorMatrix1, ccm1)
    tags.set(Tag.CameraCalibration1, camera_calibration)
    tags.set(Tag.CameraCalibration2, camera_calibration)
    tags.set(Tag.CalibrationIlluminant1, ci1)
    tags.set(Tag.AsShotNeutral, as_shot_neutral)

    # tags.set(Tag.TileWidth, data.shape[0])
    # tags.set(Tag.TileLength, data.shape[1])
    # tags.set(Tag.AsShotNeutral, [[1,1],[1,1],[1,1]])
    tags.set(Tag.DNGVersion, DNGVersion.V1_4)
    tags.set(Tag.DNGBackwardVersion, DNGVersion.V1_2)
    tags.set(Tag.PreviewColorSpace, PreviewColorSpace.sRGB)

    r = RAW2DNG()
    r.options(tags, path="", compress=False)
    r.convert(data, filename=file_path)
