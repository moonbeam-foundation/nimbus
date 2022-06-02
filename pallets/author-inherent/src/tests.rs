// Copyright 2019-2021 PureStake Inc.
// This file is part of Nimbus.

// Nimbus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Nimbus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Nimbus.  If not, see <http://www.gnu.org/licenses/>.

use crate::mock::*;

#[test]
fn kick_off_authorship_validation_is_mandatory() {
	use frame_support::weights::{DispatchClass, GetDispatchInfo};

	let info = crate::Call::<Test>::kick_off_authorship_validation {}.get_dispatch_info();
	assert_eq!(info.class, DispatchClass::Mandatory);
}

