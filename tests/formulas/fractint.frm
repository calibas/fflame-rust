comment {
  The formula language's test entries, Fractint's dialect: its shape and
  its conventions (from its parser.c). Each compiles, and on the GPU
  runs as the f64 interpreter does (escape::lang tests). Written for
  this project; no third-party formulas.
}

FrMandel {
  z = 0, c = pixel:
  z = sqr(z) + c, |z| <= 4
}

FrLastSqr {
  z = pixel:
  z = sqr(z) + p1, LastSqr <= 4
}

FrTruth {
  z = pixel, t = (0, 1), r = round((-2.5, 2.5)):
  IF (t)
    z = z * 2
  ELSEIF ((1, 5) == (1, 7))
    z = sqr(z) + pixel + r * 0.01
  ELSE
    z = z * 0.5
  ENDIF
  |z| <= 4
}

FrCopies {
  p1 = p1 + 0.1, pixel = pixel * 0.9, z = 0:
  z = sqr(z) + pixel + p1, |z| <= 4
}

FrScreen {
  z = pixel + whitesq * 0.01 + scrnpix * 0.001 + scrnmax * 0.0001
  z = z + magxmag * 0.001 + rotskew * 0.001 + center:
  IF (ismand)
    z = sqr(z) + pixel
  ELSE
    z = sqr(z) + p1
  ENDIF
  |z| <= 4
}

FrFns {
  z = pixel:
  z = fn1(z) * 0.5 + fn2(z) * 0.25 + fn3(z) * 0.1 + fn4(z) * 0.1 + cosxx(z) * 0.05 + pixel
  |z| <= 16
}

FrLoop {
  ; No init: the whole entry is the loop, its last statement the test.
  z = sqr(z) + pixel, z = z * 0.99, |z| < 4
}
