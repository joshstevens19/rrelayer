import {createClient} from "rrelayer";
import * as dotenv from "dotenv";

dotenv.config();

export const client = createClient({
    serverUrl: 'http://localhost:8000',
    auth: {
        username: process.env.RRELAYER_AUTH_USERNAME!,
        password: process.env.RRELAYER_AUTH_PASSWORD!,
    },
});
