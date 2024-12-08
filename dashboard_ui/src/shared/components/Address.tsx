import React from 'react';

const formatAddressWithEllipsis = (address: string): string => {
  if (address.length <= 10) {
    return address;
  } else {
    const start = address.substring(0, 6);
    const end = address.substring(address.length - 4);
    return `${start}...${end}`;
  }
};

const AddressComponent: React.FC<{ address: string }> = ({ address }) => {
  const formattedAddress = formatAddressWithEllipsis(address);

  return <span>{formattedAddress}</span>;
};

export default AddressComponent;
