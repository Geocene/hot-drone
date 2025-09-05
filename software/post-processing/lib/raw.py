import numpy

def imx477_raw_read(file_path: str, shape: tuple) -> numpy.ndarray:
    stride, height = shape

    data = numpy.fromfile(file_path, dtype=numpy.uint16)
    data = data.reshape((height, stride))

    return data
