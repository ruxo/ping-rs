use paste::paste;

pub(crate) const ICMP_HEADER_SIZE: usize = 8;

#[repr(C)]
pub(crate) struct IcmpEchoHeader {
    pub r#type: u8,
    pub code: u8,

    checksum: [u8;2],

    ident: [u8;2],
    seq: [u8;2],
}

macro_rules! simple_property {
    ($type:ty | $name:ident) => {
        paste! {
            pub(crate) fn $name(&self) -> $type {
                $type::from_be_bytes(self.$name)
            }
            pub(crate) fn [<set_ $name>](&mut self, $name: $type) { 
                self.$name = $name.to_be_bytes();
            }
        }
    };
}

impl IcmpEchoHeader {
    #![allow(dead_code)]

    pub(crate) fn get_mut_ref(be_buffer: &mut [u8]) -> &mut IcmpEchoHeader {
        let header = be_buffer.as_mut_ptr() as *mut IcmpEchoHeader;
        unsafe { &mut *header }
    }
    pub(crate) fn get_ref(be_buffer: &[u8]) -> &IcmpEchoHeader {
        let header = be_buffer.as_ptr() as *mut IcmpEchoHeader;
        unsafe { &*header }
    }

    simple_property![u16| checksum];
    simple_property![u16| ident];
    simple_property![u16| seq];
}

#[cfg(test)]
mod test {
    use crate::linux_ping::icmp_header::{ICMP_HEADER_SIZE, IcmpEchoHeader};

    #[test]
    fn test_encode(){
        let mut buffer = [0; ICMP_HEADER_SIZE];

        // Act
        let header = IcmpEchoHeader::get_mut_ref(&mut buffer);
        header.r#type = 1;
        header.code = 2;
        header.set_checksum(3);
        header.set_ident(4);
        header.set_seq(5);

        // Assert
        assert_eq!(buffer[0], 1); // type
        assert_eq!(buffer[1], 2); // code
        assert_eq!(buffer[2], 0); // checksum
        assert_eq!(buffer[3], 3);
        assert_eq!(buffer[4], 0); // ident
        assert_eq!(buffer[5], 4);
        assert_eq!(buffer[6], 0); // sequence
        assert_eq!(buffer[7], 5);
    }

    #[test]
    fn test_decode() {
        let buffer = [1, 2, 0, 3, 0, 4, 0, 5, 64, 24, 0, 0, 0, 0, 0, 0];

        // Act
        let header = IcmpEchoHeader::get_ref(&buffer);

        // Assert
        assert_eq!(header.r#type, 1);
        assert_eq!(header.code, 2);
        assert_eq!(header.checksum(), 3);
        assert_eq!(header.ident(), 4);
        assert_eq!(header.seq(), 5);
    }
}