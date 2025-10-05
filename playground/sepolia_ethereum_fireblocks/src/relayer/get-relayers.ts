import {client} from "../client";

export const getAllRelayer = async () => {
    const relayers = await client.relayer.getAll();
    console.log('relayers', relayers);
};

getAllRelayer().then(() => console.log('get-all-relayers done'));
