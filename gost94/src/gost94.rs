#![cfg_attr(feature = "cargo-clippy", allow(needless_range_loop))]
#![cfg_attr(feature = "cargo-clippy", allow(many_single_char_names))]
use digest::{Input, BlockInput, FixedOutput};
use block_buffer::BlockBuffer;
use block_buffer::block_padding::ZeroPadding;
use digest::generic_array::GenericArray;
use digest::generic_array::typenum::U32;
use byte_tools::{
    read_u32v_le, read_u32_le, write_u32v_le, read_u64v_le, write_u64v_le
};

pub(crate) type Block = [u8; 32];

const C: Block = [
    0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff,
    0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00,
    0x00, 0xff, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff,
    0xff, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00, 0xff,
];

pub type SBox = [[u8; 16]; 8];

fn sbox(a: u32, s: &SBox) -> u32 {
    let mut v = 0;
    for i in 0..8 {
        let shft = 4*i;
        let k = ((a & (0b1111u32 << shft) ) >> shft) as usize;
        v += u32::from(s[i][k]) << shft;
    }
    v
}

fn g(a: u32, k: u32, s: &SBox) -> u32 {
    sbox(a.wrapping_add(k), s).rotate_left(11)
}

fn encrypt(msg: &mut [u8], key: Block, sbox: &SBox) {
    let mut k = [0u32; 8];
    let mut a = read_u32_le(&msg[0..4]);
    let mut b = read_u32_le(&msg[4..8]);
    read_u32v_le(&mut k, &key);

    for _ in 0..3 {
        for i in 0..8 {
            let t = b ^ g(a, k[i], sbox);
            b = a;
            a = t;
        }
    }
    for i in (0..8).rev() {
        let t = b ^ g(a, k[i], sbox);
        b = a;
        a = t;
    }
    write_u32v_le(msg, &[b, a]);
}

fn x(a: &Block, b: &Block) -> Block {
    let mut out = Block::default();
    for i in 0..32 {
        out[i] = a[i]^b[i];
    }
    out
}

fn x_mut(a: &mut Block, b: &Block) {
    for i in 0..32 {
        a[i] ^= b[i];
    }
}


fn a(x: Block) -> Block {
    let mut out = Block::default();
    out[..24].clone_from_slice(&x[8..]);
    for i in 0..8 {
        out[24+i] = x[i]^x[i+8];
    }
    out
}

fn p(y: Block) -> Block {
    let mut out = Block::default();

    for i in 0..4 {
        for k in 0..8 {
            out[i+4*k] = y[8*i+k];
        }
    }
    out
}


fn psi(block: &mut Block) {
    let mut out = Block::default();
    out[..30].copy_from_slice(&block[2..]);
    out[30..].copy_from_slice(&block[..2]);

    out[30] ^= block[2];
    out[31] ^= block[3];

    out[30] ^= block[4];
    out[31] ^= block[5];

    out[30] ^= block[6];
    out[31] ^= block[7];

    out[30] ^= block[24];
    out[31] ^= block[25];

    out[30] ^= block[30];
    out[31] ^= block[31];

    block.copy_from_slice(&out);
}

#[derive(Clone)]
struct Gost94State {
    s: SBox,
    h: Block,
    n: [u64; 4],
    sigma: [u64; 4],
}

impl Gost94State {
    fn shuffle(&mut self, m: &Block, s: &Block) {
        let mut res = Block::default();
        res.copy_from_slice(s);
        for _ in 0..12 {
            psi(&mut res);
        }
        x_mut(&mut res, m);
        psi(&mut res);
        x_mut(&mut self.h, &res);
        for _ in 0..61 {
            psi(&mut self.h);
        }
    }

    fn f(&mut self, m: &Block) {
        let mut s = Block::default();
        s.copy_from_slice(&self.h);
        let k = p(x(&self.h, m));
        encrypt(&mut s[0..8], k, &self.s);

        let u = a(self.h);
        let v = a(a(*m));
        let k = p(x(&u, &v));
        encrypt(&mut s[8..16], k, &self.s);

        let mut u = a(u);
        x_mut(&mut u, &C);
        let v = a(a(v));
        let k = p(x(&u, &v));
        encrypt(&mut s[16..24], k, &self.s);

        let u = a(u);
        let v = a(a(v));
        let k = p(x(&u, &v));
        encrypt(&mut s[24..32], k, &self.s);

        self.shuffle(m, &s);
    }

    fn update_sigma(&mut self, m: &Block) {
        let mut buf = [0u64; 4];
        read_u64v_le(&mut buf, m);
        let mut over = (0u64, false);
        for (a, b) in self.sigma.iter_mut().zip(buf.iter()) {
            if over.1 {
                over = a.overflowing_add(*b);
                *a = over.0 + 1;
            } else {
                over = a.overflowing_add(*b);
                *a = over.0;
            }

        }
    }

    fn update_n(&mut self, len: usize) {
        let (res, over) = self.n[0].overflowing_add((len as u64) << 3);
        self.n[0] = res;
        if over {
            let (res, over) = self.n[1].overflowing_add(1 + ((len as u64) >> 61));
            self.n[1] = res;
            if over {
                let (res, over) = self.n[2].overflowing_add(1);
                self.n[2] = res;
                if over {
                    let (res, over) = self.n[3].overflowing_add(1);
                    self.n[3] = res;
                    if over { panic!("Message longer than 2^256-1")}
                }
            }
        }
    }

    fn process_block(&mut self, block: &GenericArray<u8, U32>) {
        let block = unsafe { &*(block.as_ptr() as *const [u8; 32]) };
        self.f(block);
        self.update_sigma(block);
    }
}

#[derive(Clone)]
pub struct Gost94 {
    buffer: BlockBuffer<U32>,
    state: Gost94State,
    h0: Block,
}

impl Gost94 {
    // Create new GOST94 instance with given S-Box and IV
    pub fn new(s: SBox, h: Block) -> Self {
        let n = Default::default();
        let sigma = Default::default();
        Gost94 {
            buffer: Default::default(),
            h0: h,
            state: Gost94State { s, h, n, sigma },
        }
    }
}

impl BlockInput for Gost94 {
    type BlockSize = U32;
}

impl Input for Gost94 {
    fn process(&mut self, input: &[u8]) {
        let self_state = &mut self.state;
        self_state.update_n(input.len());
        self.buffer.input(input, |d| self_state.process_block(d));
    }
}

impl FixedOutput for Gost94 {
    type OutputSize = U32;

    fn fixed_result(&mut self) -> GenericArray<u8, U32> {
        {
            let self_state = &mut self.state;

            if self.buffer.position() != 0 {
                let block = self.buffer.pad_with::<ZeroPadding>()
                    .expect("we never use input_lazy");
                self_state.process_block(block);
            }

            let mut buf = Block::default();

            write_u64v_le(&mut buf, &self_state.n);
            self_state.f(&buf);

            write_u64v_le(&mut buf, &self_state.sigma);
            self_state.f(&buf);
        }

        let res = GenericArray::clone_from_slice(&self.state.h);
        self.buffer = Default::default();
        self.state.n = Default::default();
        self.state.h = self.h0;
        self.state.sigma = Default::default();
        res
    }
}

impl_opaque_debug!(Gost94);
impl_write!(Gost94);
