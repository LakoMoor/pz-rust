use pz_sounds::wav::to_wav;

#[test]
fn wav_header_and_samples_are_laid_out_as_riff_pcm() {
    let w = to_wav(&[1, -2, 3, 4], 2, 48000);
    assert_eq!(&w[..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize, w.len() - 8);
    assert_eq!(&w[8..16], b"WAVEfmt ");
    assert_eq!(u16::from_le_bytes(w[22..24].try_into().unwrap()), 2);
    assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 48000);
    assert_eq!(u32::from_le_bytes(w[28..32].try_into().unwrap()), 48000 * 4);
    assert_eq!(&w[36..40], b"data");
    assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 8);
    assert_eq!(i16::from_le_bytes(w[46..48].try_into().unwrap()), -2);
    assert_eq!(w.len(), 44 + 8);
}
