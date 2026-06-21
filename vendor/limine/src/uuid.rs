// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Universally Unique IDentifier.
#[derive(Clone, Copy)]
pub struct Uuid {
    pub a: u32,
    pub b: u16,
    pub c: u16,
    pub d: [u8; 8],
}

impl Uuid {
    pub fn is_null(&self) -> bool {
        for i in 0..8 {
            if self.d[i] != 0 {
                return false;
            }
        }
        self.a == 0 && self.b == 0 && self.c == 0
    }
}

#[cfg(feature = "uuid")]
impl From<Uuid> for ::uuid::Uuid {
    fn from(value: Uuid) -> Self {
        ::uuid::Uuid::from_fields(value.a, value.b, value.c, &value.d)
    }
}

#[cfg(feature = "uuid")]
impl From<::uuid::Uuid> for Uuid {
    fn from(value: ::uuid::Uuid) -> Self {
        let fields = value.as_fields();
        Self {
            a: fields.0,
            b: fields.1,
            c: fields.2,
            d: fields.3.clone(),
        }
    }
}
