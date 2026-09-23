// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::coded::crosses_as_its_code;

crosses_as_its_code! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub enum ChannelType {
        #[default]
        Broadcast = 0,
        Acceleration = 1, // can be unreliable (UDP)
        Touch = 2,        // can be unreliable (UDP)
        Message = 3,
        String = 4,
        Bytes = 5,
        Gyro = 6,        // can be unreliable (UDP)
        Orientation = 7, // can be unreliable (UDP)
        DPad = 8,
    }
}
